package app.quire.notes.core.sync

import app.quire.notes.core.Md
import app.quire.notes.core.Sync
import java.io.File
import java.util.Base64
import java.util.TreeMap
import java.util.TreeSet

/** What sync needs from the settings. [cfg] holds the fields of one provider. */
data class Remote(
    val provider: String,
    val cfg: Map<String, String>,
    val prefix: String,
    val encrypt: Boolean,
    val pass: String,
) {
    fun v(k: String, d: String): String = cfg[k]?.trim()?.takeIf { it.isNotEmpty() } ?: d

    /** A stable id for this notes folder on this store, for the base record. [root] is the folder location. */
    fun id(root: String): String =
        Sign.sha256Hex("$root|$provider|${v("bucket", "")}|${v("container", "")}|$prefix").substring(0, 20)

    // The passphrase stays out of logs and error reports.
    override fun toString(): String = "Remote($provider, $prefix, encrypt=$encrypt)"
}

/** A note to write locally. */
data class Pull(
    val path: String,
    val text: String,
    /** The hash of the local text that this sync saw, or null when the note was not there. */
    val seen: String?,
    /** The device that wrote the remote text. */
    val device: String,
)

/** The size, the change time and the hash of an image, so an image that did not change is not read again. */
data class ImgStamp(val size: Long, val time: Long, val hash: String)

/**
 * What both sides had after the last sync on this device: a hash for each note and image, and
 * the folders. [stamps] are for this device only.
 */
data class Base(
    val files: Map<String, String> = emptyMap(),
    val folders: Set<String> = emptySet(),
    val stamps: Map<String, ImgStamp> = emptyMap(),
    /** The pins after the last sync. Null in a record from before pins synced. */
    val pins: Set<String>? = null,
)

/** An image file of this device. [time] is its change time, or 0 when the store does not know it. */
class LocalImage(val size: Long, val time: Long, val read: () -> ByteArray)

/** An image to write locally, with the local hash that this sync saw, and the device that wrote it. */
class ImagePull(val path: String, val bytes: ByteArray, val seen: String?, val device: String)

/** What a sync did, and what the app must still apply to the open folder. */
class Outcome(
    val pulls: List<Pull>,
    /** Notes that another device deleted, with the local hash that this sync saw. */
    val removes: List<Pair<String, String>>,
    /** Images to write here. */
    val imagePulls: List<ImagePull>,
    /** Images that another device deleted, with the local hash that this sync saw. */
    val imageRemoves: List<Pair<String, String>>,
    val uploaded: Int,
    val deletedRemote: Int,
    val conflicts: List<String>,
    /** Folders that another device made. The app makes them here. */
    val mkdirs: List<String>,
    /**
     * Folders that another device deleted, the deepest first. The app moves each one to .trash
     * when no note is in it.
     */
    val rmdirs: List<String>,
    val place: String,
    /**
     * The new base record. The app saves it with [SyncEngine.saveBase] after it applied the
     * outcome. If the app stops before that, the next sync starts from the old base, and no
     * note or folder is lost.
     */
    val base: Base,
    /** The pins after this sync. The app adds and removes what changed from the pins it sent. */
    val pins: Set<String>? = null,
)

private class Entry(val hash: String, val obj: String, val time: Long, val device: String)

private class Manifest(
    val files: TreeMap<String, Entry> = TreeMap(),
    val folders: TreeSet<String> = TreeSet(),
    /** The pinned notes. Null in a manifest from a Quire version without pins. */
    var pins: TreeSet<String>? = null,
)

/**
 * Sync: keeps a notes folder the same on every device through an object store. Follows
 * desktop/src/sync/mod.rs and SYNC.md, so the desktop app and this app share one remote folder.
 */
/** What one path needs in a sync. */
private sealed class Step {
    class Up(val p: String) : Step()
    class Down(val p: String, val e: Entry, val seen: String?) : Step()
    class Both(val p: String, val e: Entry, val copy: String) : Step()
    class DeleteRemote(val p: String, val e: Entry) : Step()
}

object SyncEngine {
    /** How many requests run at once. */
    private const val WORKERS = 8

    /**
     * [f] on every item, [WORKERS] at a time, with the results in the order of the items. The
     * first error stops the rest, and the first error in item order is the one that comes out.
     */
    private fun <T, R> parallel(items: List<T>, f: (T) -> R): List<R> {
        if (items.size <= 1) return items.map(f)
        val next = java.util.concurrent.atomic.AtomicInteger(0)
        val failed = java.util.concurrent.atomic.AtomicBoolean(false)
        val slots = arrayOfNulls<Result<R>>(items.size)
        val threads = (0 until minOf(WORKERS, items.size)).map {
            Thread {
                while (true) {
                    val i = next.getAndIncrement()
                    if (i >= items.size || failed.get()) break
                    val r = runCatching { f(items[i]) }
                    if (r.isFailure) failed.set(true)
                    slots[i] = r
                }
            }.apply {
                name = "quire-sync"
                start()
            }
        }
        threads.forEach { it.join() }
        return slots.map { r ->
            when {
                r == null -> throw SyncException("A sync step did not run.")
                r.isFailure -> throw r.exceptionOrNull()!!
                else -> r.getOrThrow()
            }
        }
    }

    private const val FORMAT = ".quire/format.json"

    /** Larger images neither insert nor sync. */
    const val MAX_BYTES = app.quire.notes.core.Images.MAX_BYTES

    /** The remote folder inside the bucket. It always ends in "/", unless it is empty. */
    fun cleanPrefix(p: String): String {
        val t = p.trim().trim('/')
        return if (t.isEmpty()) "" else "$t/"
    }

    /** The remote folder from the settings, or quire/<folder name>/. */
    fun prefixFor(cfg: Map<String, String>, folderName: String): String =
        cleanPrefix(cfg["prefix"]?.takeIf { it.isNotBlank() } ?: "quire/$folderName/")

    /** The region that the settings show first for a provider, used until one is picked. */
    private fun defaultRegion(pid: String): String = Sync.provider(pid)?.regions?.firstOrNull()?.first ?: "us-east-1"

    fun openStore(r: Remote): ObjStore {
        val pid = r.provider
        val region = r.v("region", defaultRegion(pid))
        fun s3(endpoint: String, signRegion: String, pathStyle: Boolean, label: String = "secret access key"): ObjStore =
            S3(endpoint, r.v("bucket", ""), signRegion, r.v("accessKey", ""), r.v("secretKey", ""), pathStyle, r.prefix, label)
        return when (pid) {
            "s3" -> s3("https://s3.$region.amazonaws.com", region, false)
            "hetzner" -> s3("https://$region.your-objectstorage.com", region, true)
            "r2" -> s3("https://${r.v("accountId", "")}.r2.cloudflarestorage.com", "auto", true)
            "b2" -> s3("https://s3.$region.backblazeb2.com", region, true, "application key")
            "do" -> s3("https://$region.digitaloceanspaces.com", "us-east-1", true)
            "wasabi" -> s3("https://s3.$region.wasabisys.com", region, true)
            "scaleway" -> s3("https://s3.$region.scw.cloud", region, true)
            "ovh" -> s3("https://s3.$region.io.cloud.ovh.net", region, true)
            "custom" -> s3(r.v("endpoint", ""), r.v("region", "us-east-1"), r.v("addressing", "path") != "vhost")
            "azure" -> Azure(r.v("account", ""), r.v("container", ""), r.v("azAuth", "key"), r.v("azKey", ""), r.prefix)
            "gcs" -> if (r.v("gcsAuth", "sa") == "hmac") {
                S3("https://storage.googleapis.com", r.v("bucket", ""), "auto", r.v("hmacId", ""), r.v("hmacSecret", ""), true, r.prefix, "HMAC secret")
            } else {
                Gcs(r.v("bucket", ""), r.v("gcsJson", ""), r.prefix)
            }
            else -> throw SyncException("Pick a storage backend first.")
        }
    }

    private fun checkPass(r: Remote) {
        if (r.encrypt && r.pass.codePointCount(0, r.pass.length) < 12) throw SyncException("The passphrase needs at least 12 characters.")
    }

    /** Reads the format of the remote folder, or writes it when [create] is true. Returns the keys when encrypted. */
    private fun openFormat(store: ObjStore, r: Remote, create: Boolean): Keys? {
        val bytes = store.get(FORMAT)
        if (bytes == null) {
            if (!create) return null
            val f = LinkedHashMap<String, Any?>()
            f["version"] = 1
            f["encrypted"] = r.encrypt
            var keys: Keys? = null
            if (r.encrypt) {
                val salt = Crypto.random(16)
                val k = Crypto.derive(r.pass, salt, Crypto.ITERATIONS)
                val b = Base64.getEncoder()
                f["kdf"] = "pbkdf2-sha256"
                f["iterations"] = Crypto.ITERATIONS
                f["salt"] = b.encodeToString(salt)
                f["check"] = b.encodeToString(Crypto.seal(k, "quire".toByteArray()))
                keys = k
            }
            store.put(FORMAT, Json.write(f, pretty = true).toByteArray(Charsets.UTF_8))
            return keys
        }
        val bad = "The remote folder has a format file Quire cannot read."
        val f = try {
            Json.parse(String(bytes, Charsets.UTF_8)) as? Map<*, *>
        } catch (e: Exception) {
            null
        } ?: throw SyncException(bad)
        val encrypted = f["encrypted"] as? Boolean ?: throw SyncException(bad)
        if (encrypted && !r.encrypt) throw SyncException("The remote notes are encrypted. Turn on Encrypt before upload and enter the passphrase.")
        if (!encrypted && r.encrypt) throw SyncException("The remote notes are not encrypted. Turn off Encrypt before upload, or use a new remote folder.")
        if (!encrypted) return null
        val dec = Base64.getDecoder()
        val salt = (f["salt"] as? String)?.let { runCatching { dec.decode(it) }.getOrNull() } ?: throw SyncException("The remote format has no salt.")
        val iterations = when (val it = f["iterations"]) {
            null -> Crypto.ITERATIONS.toLong()
            is Long -> it
            else -> throw SyncException(bad)
        }
        if (iterations < 1 || iterations > 50_000_000) throw SyncException(bad)
        val k = Crypto.derive(r.pass, salt, iterations.toInt())
        val check = (f["check"] as? String)?.let { runCatching { dec.decode(it) }.getOrNull() } ?: throw SyncException("The remote format has no check value.")
        val ok = try {
            Crypto.open(k, check).contentEquals("quire".toByteArray())
        } catch (e: SyncException) {
            false
        }
        if (!ok) throw SyncException("The passphrase does not match the one these notes were encrypted with.")
        return k
    }

    private fun manifestKey(keys: Keys?) = if (keys != null) ".quire/manifest.bin" else ".quire/manifest.json"

    private fun readManifest(store: ObjStore, keys: Keys?): Manifest {
        val bytes = store.get(manifestKey(keys)) ?: return Manifest()
        val plain = if (keys != null) Crypto.open(keys, bytes) else bytes
        val bad = SyncException("The remote manifest could not be read.")
        val m = try {
            Json.parse(String(plain, Charsets.UTF_8)) as? Map<*, *>
        } catch (e: Exception) {
            null
        } ?: throw bad
        val files = m["files"] as? Map<*, *> ?: throw bad
        val out = TreeMap<String, Entry>()
        for ((p, e) in files) {
            val o = e as? Map<*, *> ?: throw bad
            val hash = o["hash"] as? String ?: throw bad
            val obj = o["object"] as? String ?: throw bad
            val time = (o["time"] as? Number)?.toLong() ?: throw bad
            val device = o["device"] as? String ?: throw bad
            out[p.toString()] = Entry(hash, obj, time, device)
        }
        // A manifest from before folders synced has no folders.
        val dirs = TreeSet<String>()
        when (val f = m["folders"]) {
            null -> {}
            is List<*> -> for (d in f) dirs.add(d as? String ?: throw bad)
            else -> throw bad
        }
        // A manifest from a version without pins says nothing about pins.
        val pins = when (val f = m["pins"]) {
            null -> null
            is List<*> -> f.mapTo(TreeSet()) { it as? String ?: throw bad }
            else -> throw bad
        }
        return Manifest(out, dirs, pins)
    }

    private fun writeManifest(store: ObjStore, keys: Keys?, m: Manifest) {
        val files = LinkedHashMap<String, Any?>()
        for ((p, e) in m.files) files[p] = linkedMapOf("hash" to e.hash, "object" to e.obj, "time" to e.time, "device" to e.device)
        val body = linkedMapOf<String, Any?>("version" to 1, "files" to files, "folders" to m.folders.toList())
        // The key stays out when the manifest has no pins, as the desktop app writes it.
        m.pins?.let { body["pins"] = it.toList() }
        val json = Json.write(body).toByteArray(Charsets.UTF_8)
        store.put(manifestKey(keys), if (keys != null) Crypto.seal(keys, json) else json)
    }

    /** Checks the settings against the store, as Test connection does. It only reads. */
    fun test(r: Remote, localNotes: Int, syncedBefore: Boolean, store: ObjStore? = null): String {
        checkPass(r)
        val s = store ?: openStore(r)
        val keys = openFormat(s, r, false)
        val place = s.place
        val files = if (s.get(manifestKey(keys)) != null) readManifest(s, keys).files else TreeMap()
        val notes = files.keys.count { !Md.isImage(it) }
        val images = files.size - notes
        fun plural(n: Int, one: String, many: String) = "$n ${if (n == 1) one else many}"
        val held = if (images > 0) "${plural(notes, "note", "notes")} and ${plural(images, "image", "images")}" else plural(notes, "note", "notes")
        val folder = if (r.prefix.isEmpty()) "The bucket" else "The remote folder ${r.prefix.trimEnd('/')}"
        return when {
            // A first sync with notes on both sides joins them. Say so, so a remote folder that
            // belongs to another notes folder is easy to see.
            files.isNotEmpty() && !syncedBefore ->
                "Connected to $place. $folder holds $held, and this folder has ${plural(localNotes, "note", "notes")}. The first sync brings them together."
            files.isNotEmpty() -> "Connected to $place. It holds $held."
            syncedBefore -> "Connected to $place. Everything is up to date."
            else -> "Connected to $place. $localNotes ${if (localNotes == 1) "note" else "notes"} will upload on the first sync."
        }
    }

    /**
     * A path that may sync: a note or an image, with no ".." part, no empty part and no part
     * that starts with ".". Other paths are skipped, as on the desktop.
     */
    fun syncable(p: String): Boolean {
        if (!(p.lowercase().endsWith(".md") || Md.isImage(p)) || p.contains('\u0000')) return false
        return p.split('/').all { it.isNotEmpty() && it != ".." && !it.startsWith(".") }
    }

    /** A folder that may sync: no part that starts with "." and no node_modules. */
    fun syncableDir(p: String): Boolean {
        if (p.contains('\\') || p.contains('\u0000')) return false
        return p.split('/').all { it.isNotEmpty() && !it.startsWith(".") && it != "node_modules" }
    }

    /** The path of a conflict copy beside [p]: "name (conflict device).md". An image keeps its extension. */
    fun conflictPath(p: String, device: String, taken: Set<String>): String {
        val dir = Md.dir(p)
        val file = p.substringAfterLast('/')
        val i = file.lastIndexOf('.')
        val stem = if (i >= 0) file.substring(0, i) else file
        val ext = if (i >= 0) file.substring(i + 1) else "md"
        val dev = device.replace(Regex("""[\\/:]"""), "-").trim()
        var c = Md.join(dir, "$stem (conflict $dev).$ext")
        var n = 2
        while (c in taken) c = Md.join(dir, "$stem (conflict $dev ${n++}).$ext")
        return c
    }

    fun loadBase(file: File?): Base {
        if (file == null || !file.isFile) return Base()
        return try {
            val m = Json.parse(file.readText()) as? Map<*, *> ?: return Base()
            fun strings(v: Any?) = (v as? Map<*, *>)?.entries?.mapNotNull { (k, h) -> (h as? String)?.let { k.toString() to it } }?.toMap() ?: emptyMap()
            val files = m["files"]
            // A record from before folders synced is a plain map of notes. A note path ends in .md, so it is never "files".
            if (files is Map<*, *>) {
                val stamps = HashMap<String, ImgStamp>()
                (m["stamps"] as? Map<*, *>)?.forEach { (k, v) ->
                    val l = v as? List<*>
                    val size = l?.getOrNull(0) as? Long
                    val time = l?.getOrNull(1) as? Long
                    val hash = l?.getOrNull(2) as? String
                    if (size != null && time != null && hash != null) stamps[k.toString()] = ImgStamp(size, time, hash)
                }
                val pins = (m["pins"] as? List<*>)?.mapNotNull { it as? String }?.toSet()
                Base(strings(files), (m["folders"] as? List<*>)?.mapNotNull { it as? String }?.toSet() ?: emptySet(), stamps, pins)
            } else {
                Base(strings(m))
            }
        } catch (e: Exception) {
            Base()
        }
    }

    fun saveBase(file: File, base: Base) {
        file.parentFile?.mkdirs()
        val tmp = File(file.parentFile, file.name + ".tmp")
        val stamps = TreeMap<String, Any?>()
        for ((p, st) in base.stamps) stamps[p] = listOf(st.size, st.time, st.hash)
        val body = linkedMapOf<String, Any?>("files" to TreeMap(base.files), "folders" to base.folders.sorted(), "stamps" to stamps)
        base.pins?.let { body["pins"] = it.sorted() }
        tmp.writeText(Json.write(body))
        if (!tmp.renameTo(file)) {
            file.writeText(tmp.readText())
            tmp.delete()
        }
    }

    /** One sync. Checks the passphrase and opens the store from the settings first. */
    fun run(
        r: Remote,
        local: Map<String, String>,
        localImages: Map<String, LocalImage>,
        localDirs: Set<String>,
        device: String,
        base: Base,
        pins: Set<String> = emptySet(),
    ): Outcome {
        checkPass(r)
        return run(openStore(r), r, local, localImages, localDirs, device, base, pins)
    }

    /**
     * Merges pins as folders merge: a pin added on one side is added, and a pin removed on one
     * side is removed. [remote] is null when the manifest comes from a version without pins:
     * the pins here stay. [base] is null before the first sync with pins: nothing is removed.
     */
    fun mergePins(local: Set<String>, remote: Set<String>?, base: Set<String>?): Set<String> {
        if (remote == null) return TreeSet(local)
        if (base == null) return TreeSet(local + remote)
        return (local + remote).filterTo(TreeSet()) { p ->
            // On one side only: added there, or removed on the other side.
            (p in local && p in remote) || p !in base
        }
    }

    /**
     * One sync of a notes folder. [local] holds every saved note of the folder, [localDirs]
     * holds its folders, and [base] holds what both sides had after the last sync on this device.
     * [pins] are the pinned notes of the folder now.
     */
    fun run(
        store: ObjStore,
        r: Remote,
        local: Map<String, String>,
        localImages: Map<String, LocalImage>,
        localDirs: Set<String>,
        device: String,
        base: Base,
        pins: Set<String> = emptySet(),
    ): Outcome {
        val keys = openFormat(store, r, true)
        val m = readManifest(store, keys)
        val manifest = m.files
        val baseDirs = base.folders
        val baseFiles = base.files
        val pulls = ArrayList<Pull>()
        val removes = ArrayList<Pair<String, String>>()
        val imagePulls = ArrayList<ImagePull>()
        val imageRemoves = ArrayList<Pair<String, String>>()
        val conflicts = ArrayList<String>()
        var uploaded = 0
        var deletedRemote = 0
        val newBase = TreeMap<String, String>()
        val sets = LinkedHashMap<String, Entry>()
        val dels = LinkedHashSet<String>()
        val now = System.currentTimeMillis()

        // Notes by their text, images by their bytes. An image keeps its hash while its size and
        // change time stay the same, so an image that did not change is not read again.
        class Here(val note: String?, val image: LocalImage?, val hash: String)
        val here = TreeMap<String, Here>()
        for ((p, t) in local) here[p] = Here(t, null, Sign.sha256Hex(t))
        val stamps = TreeMap<String, ImgStamp>()
        for ((p, img) in localImages) {
            if (img.size > MAX_BYTES) continue
            val old = base.stamps[p]
            val h = if (old != null && img.time > 0 && old.size == img.size && old.time == img.time) {
                old.hash
            } else {
                try {
                    Sign.sha256Hex(img.read())
                } catch (e: Exception) {
                    continue
                }
            }
            stamps[p] = ImgStamp(img.size, img.time, h)
            here[p] = Here(null, img, h)
        }

        val paths = TreeSet<String>()
        paths.addAll(here.keys)
        paths.addAll(manifest.keys)
        paths.addAll(baseFiles.keys)
        val taken = HashSet(paths)
        paths.removeAll { !syncable(it) }

        fun body(h: Here): ByteArray = h.note?.toByteArray(Charsets.UTF_8) ?: h.image!!.read()

        // What comes down: the text of a note, or the bytes of an image.
        fun give(p: String, data: ByteArray, seen: String?, from: String) {
            if (Md.isImage(p)) {
                imagePulls.add(ImagePull(p, data, seen, from))
            } else {
                pulls.add(Pull(p, String(data, Charsets.UTF_8).replace("\r\n", "\n"), seen, from))
            }
        }

        // 1. Decide what each path needs, without the network.
        val steps = ArrayList<Step>()
        for (p in paths) {
            val l = here[p]
            val e = manifest[p]
            val b = baseFiles[p]
            when {
                l != null && e != null && l.hash == e.hash -> newBase[p] = l.hash
                l != null && e == null && b == l.hash -> {
                    // Another device deleted it, and this one did not change it.
                    if (Md.isImage(p)) imageRemoves.add(p to l.hash) else removes.add(p to l.hash)
                }
                l != null && e == null -> steps.add(Step.Up(p))
                l == null && e != null && b == e.hash -> steps.add(Step.DeleteRemote(p, e))
                l == null && e != null -> steps.add(Step.Down(p, e, null))
                l == null -> {}
                e != null && b == e.hash -> steps.add(Step.Up(p))
                e != null && b == l.hash -> steps.add(Step.Down(p, e, l.hash))
                e != null -> {
                    // Both sides changed: the remote version comes down and goes up under a new
                    // name, and the version of this device goes up at the path.
                    val c = conflictPath(p, e.device, taken)
                    taken.add(c)
                    steps.add(Step.Both(p, e, c))
                }
            }
        }

        // 2. Downloads, 8 at a time.
        val wanted = steps.mapNotNull {
            when (it) {
                is Step.Down -> it.e
                is Step.Both -> it.e
                else -> null
            }
        }
        val gotList = parallel(wanted) { e ->
            val bytes = store.get(e.obj)
            if (bytes == null) null else if (keys != null) Crypto.open(keys, bytes) else bytes
        }
        val got = HashMap<String, ByteArray>()
        wanted.zip(gotList).forEach { (e, b) -> if (b != null) got[e.obj] = b }

        // 3. Lay out the uploads and the outcome. A body is read when its upload runs.
        val ups = ArrayList<Pair<String, () -> ByteArray>>()
        val gone = ArrayList<Pair<String, Entry>>()
        for (st in steps) {
            when (st) {
                is Step.Up -> {
                    val h = here.getValue(st.p)
                    ups.add(st.p to { body(h) })
                }
                is Step.Down -> got.remove(st.e.obj)?.let {
                    newBase[st.p] = st.e.hash
                    give(st.p, it, st.seen, st.e.device)
                }
                is Step.Both -> {
                    got.remove(st.e.obj)?.let {
                        conflicts.add(st.copy)
                        // The copy goes up at once, so every device sees the conflict.
                        ups.add(st.copy to { it })
                        give(st.copy, it, null, st.e.device)
                    }
                    val h = here.getValue(st.p)
                    ups.add(st.p to { body(h) })
                }
                is Step.DeleteRemote -> gone.add(st.p to st.e)
            }
        }

        // 4. Uploads and deletes, 8 at a time.
        val sent = parallel(ups) { (p, read) ->
            val data = read()
            val obj = if (keys != null) Crypto.objectName(keys, p) else p
            store.put(obj, if (keys != null) Crypto.seal(keys, data) else data)
            Entry(Sign.sha256Hex(data), obj, now, device)
        }
        ups.zip(sent).forEach { (u, e) ->
            sets[u.first] = e
            newBase[u.first] = e.hash
            uploaded++
        }
        parallel(gone) { (_, e) -> store.delete(e.obj) }
        for ((p, _) in gone) {
            dels.add(p)
            deletedRemote++
        }

        // Folders, after the notes: a folder stays while a note is left in it.
        val removed = (removes + imageRemoves).map { it.first }.toSet()
        val kept = HashSet<String>()
        here.keys.filterTo(kept) { it !in removed }
        pulls.mapTo(kept) { it.path }
        imagePulls.mapTo(kept) { it.path }
        kept.addAll(sets.keys)
        fun holdsNotes(f: String) = kept.any { Md.inDir(it, f) }
        val dirsHere = localDirs.filter { syncableDir(it) }.toSet()
        val there = m.folders.filter { syncableDir(it) }.toSet()
        val allDirs = TreeSet<String>()
        allDirs.addAll(dirsHere)
        allDirs.addAll(there)
        allDirs.addAll(baseDirs.filter { syncableDir(it) })
        val newDirs = TreeSet<String>()
        val mkdirs = ArrayList<String>()
        val rmdirs = ArrayList<String>()
        val addDirs = ArrayList<String>()
        val dropDirs = ArrayList<String>()
        for (f in allDirs) {
            val before = f in baseDirs
            when {
                f in dirsHere && f in there -> newDirs.add(f)
                f in dirsHere && before && !holdsNotes(f) -> rmdirs.add(f)
                f in dirsHere -> {
                    addDirs.add(f)
                    newDirs.add(f)
                }
                f in there && before && !holdsNotes(f) -> dropDirs.add(f)
                f in there -> {
                    mkdirs.add(f)
                    newDirs.add(f)
                }
            }
        }
        // The deepest folder goes first, so a parent is empty when its turn comes.
        rmdirs.sortByDescending { d -> d.count { it == '/' } }

        // Pins, after the notes: a pin for a note that is gone goes too.
        val newPins = TreeSet(mergePins(pins, m.pins, base.pins))
        newPins.retainAll(kept)
        val pinsThere = m.pins ?: TreeSet()
        val addPins = ArrayList<String>()
        val dropPins = ArrayList<String>()
        // A manifest without pins gets them only when there is a pin to write.
        if (m.pins != null || newPins.isNotEmpty()) {
            newPins.filterTo(addPins) { it !in pinsThere }
            pinsThere.filterTo(dropPins) { it !in newPins }
        }

        if (sets.isNotEmpty() || dels.isNotEmpty() || addDirs.isNotEmpty() || dropDirs.isNotEmpty() || addPins.isNotEmpty() || dropPins.isNotEmpty()) {
            // Read the manifest again just before the write, so a device that synced meanwhile
            // keeps its entries. Then lay the changes of this sync over it.
            val fresh = readManifest(store, keys)
            for ((p, e) in sets) fresh.files[p] = e
            for (p in dels) fresh.files.remove(p)
            fresh.folders.addAll(addDirs)
            fresh.folders.removeAll(dropDirs.toSet())
            if (addPins.isNotEmpty()) fresh.pins = (fresh.pins ?: TreeSet()).also { it.addAll(addPins) }
            fresh.pins?.removeAll(dropPins.toSet())
            writeManifest(store, keys, fresh)
        }
        stamps.keys.retainAll(newBase.keys)
        return Outcome(
            pulls, removes, imagePulls, imageRemoves, uploaded, deletedRemote, conflicts, mkdirs, rmdirs, store.place,
            Base(newBase, newDirs, stamps, newPins), newPins,
        )
    }

    /** The changes to make in the open folder after a sync. */
    class Apply(
        val writes: List<Pair<String, String>>,
        val deletes: List<String>,
        val conflicts: List<String>,
        val mkdirs: List<String>,
        val rmdirs: List<String>,
    )

    /**
     * Lays the outcome of a sync over the open folder. [current] is the text of each note now,
     * and [dirty] holds the notes with edits that are not on disk yet. A note that changed while
     * the sync ran keeps its new text, and the remote text goes beside it as a conflict copy. A
     * note that another device deleted stays when it changed here. The next sync uploads both.
     * A folder that another device deleted stays when a note is in it now. [currentDirs] holds
     * the folders now.
     */
    fun plan(out: Outcome, current: Map<String, String>, currentDirs: Set<String>, dirty: Set<String>): Apply {
        val conflicts = ArrayList(out.conflicts)
        val taken = HashSet(current.keys)
        out.pulls.forEach { taken.add(it.path) }
        val writes = ArrayList<Pair<String, String>>()
        for (pl in out.pulls) {
            val cur = current[pl.path]?.let { Sign.sha256Hex(it) }
            if (cur == Sign.sha256Hex(pl.text)) continue
            val path = if (pl.path in dirty || cur != pl.seen) {
                conflictPath(pl.path, pl.device, taken).also {
                    taken.add(it)
                    conflicts.add(it)
                }
            } else {
                pl.path
            }
            writes.add(path to pl.text)
        }
        val deletes = out.removes.filter { (p, seen) -> p !in dirty && current[p]?.let { Sign.sha256Hex(it) } == seen }.map { it.first }
        val notes = HashSet(current.keys)
        writes.forEach { notes.add(it.first) }
        notes.removeAll(deletes.toSet())
        val mkdirs = out.mkdirs.filter { it !in currentDirs }
        val rmdirs = out.rmdirs.filter { d -> d in currentDirs && notes.none { Md.inDir(it, d) } }
        return Apply(writes, deletes, conflicts, mkdirs, rmdirs)
    }

    /** The image changes to make in the folder after a sync. */
    class ImageApply(val writes: List<Pair<String, ByteArray>>, val deletes: List<String>, val conflicts: List<String>)

    /**
     * The image part of the outcome. [hashNow] gives the hash of the file at a path now, or null
     * when no file is there. An image is written when the file here is still what the sync saw,
     * or else as a conflict copy beside it. A delete from another device applies only to an
     * image that did not change. [imagesNow] holds the image paths now, for free copy names.
     */
    fun planImages(out: Outcome, imagesNow: Set<String>, hashNow: (String) -> String?): ImageApply {
        val taken = HashSet(imagesNow)
        val writes = ArrayList<Pair<String, ByteArray>>()
        val conflicts = ArrayList<String>()
        for (pl in out.imagePulls) {
            val cur = hashNow(pl.path)
            val path = if (cur != null && cur != pl.seen) {
                conflictPath(pl.path, pl.device, taken).also {
                    taken.add(it)
                    conflicts.add(it)
                }
            } else {
                pl.path
            }
            taken.add(path)
            writes.add(path to pl.bytes)
        }
        val deletes = out.imageRemoves.filter { (p, seen) -> hashNow(p) == seen }.map { it.first }
        return ImageApply(writes, deletes, conflicts)
    }

    /** "just now", "4m ago", "2h ago" or "3d ago". */
    fun ago(ms: Long, now: Long = System.currentTimeMillis()): String {
        val d = maxOf(now - ms, 0L) / 1000.0
        return when {
            d < 45 -> "just now"
            d < 3600 -> "${Math.round(d / 60)}m ago"
            d < 86_400 -> "${Math.round(d / 3600)}h ago"
            else -> "${Math.round(d / 86_400)}d ago"
        }
    }
}
