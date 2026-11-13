package app.quire.notes.core.sync

import app.quire.notes.data.LocalFs
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.io.File
import java.nio.file.Files
import java.util.TreeMap

/** A bucket in memory, shared by the devices of a test. */
class Mem : ObjStore {
    val objects = TreeMap<String, ByteArray>()
    override fun get(key: String): ByteArray? = synchronized(objects) { objects[key]?.copyOf() }
    override fun put(key: String, body: ByteArray) = synchronized(objects) { objects[key] = body.copyOf() }
    override fun delete(key: String) {
        synchronized(objects) { objects.remove(key) }
    }
    override val place: String get() = "mem"
}

/** A notes folder on disk with its own base record, as one device has it. */
class Device(tag: String, val name: String) {
    val root: File = Files.createTempDirectory("quire-sync-$tag-$name").toFile()
    private val baseFile = File(root.parentFile, root.name + ".base.json")

    fun write(p: String, t: String) {
        val f = File(root, p)
        f.parentFile?.mkdirs()
        f.writeText(t)
    }

    fun read(p: String): String? = File(root, p).takeIf { it.isFile }?.readText()

    fun remove(p: String) = assertTrue(File(root, p).delete())

    fun mkdir(p: String) = assertTrue(File(root, p).mkdirs())

    fun rmdir(p: String) = assertTrue(File(root, p).deleteRecursively())

    fun isDir(p: String) = File(root, p).isDirectory

    /** How many times a sync read the bytes of an image. */
    var reads = 0

    /** The notes and folders as the app loads them, with the file system of the app. */
    fun local(): Pair<Map<String, String>, Set<String>> = LocalFs(root).load().let { it.files to it.folders }

    fun bytes(p: String): ByteArray? = File(root, p).takeIf { it.isFile }?.readBytes()

    fun run(store: ObjStore, r: Remote, pins: Set<String> = emptySet()): Outcome {
        val l = LocalFs(root).load()
        val images = l.images.mapValues { (p, st) ->
            LocalImage(st.size, st.time) {
                reads++
                File(root, p).readBytes()
            }
        }
        return SyncEngine.run(store, r, l.files, images, l.folders, name, SyncEngine.loadBase(baseFile), pins)
    }

    /** Syncs with [pins], applies the outcome, and gives the pins after the sync. */
    fun syncPins(store: ObjStore, r: Remote, pins: Set<String>): Set<String> = sync(store, r, pins = pins).pins!!

    /** Syncs, then applies the outcome as the app does, then saves the base. */
    fun sync(store: ObjStore, r: Remote, dirty: Set<String> = emptySet(), pins: Set<String> = emptySet()): Outcome {
        val out = run(store, r, pins)
        val l = LocalFs(root).load()
        val plan = SyncEngine.plan(out, l.files, l.folders, dirty)
        val pics = SyncEngine.planImages(out, l.images.keys) { p -> bytes(p)?.let { Sign.sha256Hex(it) } }
        val fs = LocalFs(root)
        for ((p, t) in plan.writes) fs.write(p, t)
        for (p in plan.deletes) fs.delete(p)
        for ((p, b) in pics.writes) fs.writeBytes(p, b)
        for (p in pics.deletes) fs.delete(p)
        for (d in plan.mkdirs) fs.mkdirs(d)
        for (d in plan.rmdirs) fs.deleteDir(d)
        SyncEngine.saveBase(baseFile, out.base)
        return out
    }
}

class SyncEngineTest {
    private fun remote(encrypt: Boolean, pass: String = "a long enough passphrase") = Remote("mem", emptyMap(), "", encrypt, pass)

    private fun twoDevices(encrypt: Boolean, tag: String) {
        val bucket = Mem()
        val r = remote(encrypt)
        val laptop = Device(tag, "laptop")
        val phone = Device(tag, "phone")
        laptop.write("Ideas.md", "# Ideas")
        laptop.write("Drafts/Plan.md", "# Plan")
        assertEquals(2, laptop.sync(bucket, r).uploaded)
        // A second device gets both notes.
        val first = phone.sync(bucket, r)
        assertEquals(2, first.pulls.size)
        assertEquals("# Plan", phone.read("Drafts/Plan.md"))
        // An edit on the phone reaches the laptop.
        phone.write("Ideas.md", "# Ideas\n\nfrom the phone")
        assertEquals(1, phone.sync(bucket, r).uploaded)
        laptop.sync(bucket, r)
        assertEquals("# Ideas\n\nfrom the phone", laptop.read("Ideas.md"))
        // A delete on the laptop removes the note on the phone.
        laptop.remove("Drafts/Plan.md")
        assertEquals(1, laptop.sync(bucket, r).deletedRemote)
        assertEquals(listOf("Drafts/Plan.md" to Sign.sha256Hex("# Plan")), phone.sync(bucket, r).removes)
        assertNull(phone.read("Drafts/Plan.md"))
        // Both change the same note: each keeps its own text and gets the other beside it.
        laptop.write("Ideas.md", "laptop text")
        phone.write("Ideas.md", "phone text")
        laptop.sync(bucket, r)
        val out = phone.sync(bucket, r)
        assertEquals(listOf("Ideas (conflict laptop).md"), out.conflicts)
        assertEquals("phone text", phone.read("Ideas.md"))
        assertEquals("laptop text", phone.read("Ideas (conflict laptop).md"))
        laptop.sync(bucket, r)
        assertEquals("phone text", laptop.read("Ideas.md"))
        assertEquals("laptop text", laptop.read("Ideas (conflict laptop).md"))
        // An empty folder and its subfolder reach the other device, and so does their delete.
        laptop.mkdir("Archive/2025")
        laptop.sync(bucket, r)
        assertEquals(listOf("Archive", "Archive/2025"), phone.sync(bucket, r).mkdirs)
        assertTrue(phone.isDir("Archive/2025"))
        laptop.rmdir("Archive")
        laptop.sync(bucket, r)
        assertEquals(listOf("Archive/2025", "Archive"), phone.sync(bucket, r).rmdirs)
        assertFalse(phone.isDir("Archive"))
        assertTrue("the folder went to the trash", phone.isDir(".trash/Archive"))
        // A folder that gets a new note on one device while the other deletes it stays, with the note.
        phone.rmdir("Drafts")
        laptop.write("Drafts/New.md", "# New")
        phone.sync(bucket, r)
        val kept = laptop.sync(bucket, r)
        assertTrue("a folder with a new note stays: ${kept.rmdirs}", kept.rmdirs.isEmpty())
        val back = phone.sync(bucket, r)
        assertEquals(listOf("Drafts/New.md"), back.pulls.map { it.path })
        assertEquals("# New", phone.read("Drafts/New.md"))
        // A sync with nothing new changes nothing.
        val quiet = phone.sync(bucket, r)
        assertEquals(Triple(0, 0, 0), Triple(quiet.uploaded, quiet.pulls.size, quiet.removes.size))
        assertEquals(Triple(0, 0, 0), Triple(quiet.deletedRemote, quiet.mkdirs.size, quiet.rmdirs.size))
        assertEquals(laptop.local(), phone.local())
        val keys = bucket.objects.keys
        if (encrypt) {
            assertTrue("object names hide the paths: $keys", keys.none { it.contains("Ideas") })
            assertTrue("text is encrypted", bucket.objects.values.none { String(it, Charsets.UTF_8).contains("phone text") })
            assertTrue(keys.toString(), ".quire/manifest.bin" in keys && ".quire/manifest.json" !in keys)
            val format = Json.parse(String(bucket.objects.getValue(".quire/format.json"))) as Map<*, *>
            assertEquals(true, format["encrypted"])
            assertEquals("pbkdf2-sha256", format["kdf"])
            assertEquals(600_000L, format["iterations"])
        } else {
            assertEquals(setOf(".quire/format.json", ".quire/manifest.json", "Ideas.md", "Ideas (conflict laptop).md", "Drafts/New.md"), keys.toSet())
            val m = Json.parse(String(bucket.objects.getValue(".quire/manifest.json"))) as Map<*, *>
            val e = (m["files"] as Map<*, *>)["Ideas.md"] as Map<*, *>
            assertEquals(Sign.sha256Hex("phone text"), e["hash"])
            assertEquals("Ideas.md", e["object"])
            assertEquals("phone", e["device"])
            assertTrue(e["time"] is Long)
            assertEquals(1L, m["version"])
            assertEquals(listOf("Drafts"), m["folders"])
        }
    }

    @Test
    fun twoDevicesStayInStep() = twoDevices(false, "plain")

    @Test
    fun twoDevicesStayInStepEncrypted() = twoDevices(true, "enc")

    @Test
    fun aWrongPassphraseIsRefused() {
        val bucket = Mem()
        val d = Device("pass", "laptop")
        d.write("A.md", "a")
        d.sync(bucket, remote(true))
        val e = Device("pass2", "phone")
        try {
            e.run(bucket, remote(true, "a different passphrase"))
            fail("a wrong passphrase synced")
        } catch (x: SyncException) {
            assertTrue(x.message, x.message!!.contains("passphrase"))
        }
        try {
            e.run(bucket, remote(false))
            fail("a plain device synced with an encrypted folder")
        } catch (x: SyncException) {
            assertTrue(x.message, x.message!!.contains("encrypted"))
        }
        try {
            SyncEngine.run(remote(true, "short"), emptyMap(), emptyMap(), emptySet(), "phone", Base())
            fail("a short passphrase synced")
        } catch (x: SyncException) {
            assertEquals("The passphrase needs at least 12 characters.", x.message)
        }
    }

    @Test
    fun aPlainFolderRefusesAnEncryptedDevice() {
        val bucket = Mem()
        Device("plainfmt", "laptop").also { it.write("A.md", "a") }.sync(bucket, remote(false))
        try {
            Device("plainfmt2", "phone").run(bucket, remote(true))
            fail("an encrypted device synced with a plain folder")
        } catch (x: SyncException) {
            assertEquals("The remote notes are not encrypted. Turn off Encrypt before upload, or use a new remote folder.", x.message)
        }
    }

    @Test
    fun aNoteThatChangedDuringTheSyncKeepsItsText() {
        val bucket = Mem()
        val r = remote(false)
        val laptop = Device("guard", "laptop")
        val phone = Device("guard", "phone")
        laptop.write("A.md", "one")
        laptop.write("B.md", "bee")
        laptop.sync(bucket, r)
        phone.sync(bucket, r)
        laptop.write("A.md", "two")
        laptop.remove("B.md")
        laptop.sync(bucket, r)
        // The phone syncs, but the person edits both notes before the outcome lands.
        val out = phone.run(bucket, r)
        assertEquals(listOf("A.md"), out.pulls.map { it.path })
        assertEquals(listOf("B.md"), out.removes.map { it.first })
        phone.write("A.md", "one, edited")
        val plan = SyncEngine.plan(out, phone.local().first, emptySet(), setOf("B.md"))
        assertEquals(listOf("A (conflict laptop).md" to "two"), plan.writes)
        assertEquals(emptyList<String>(), plan.deletes)
        assertEquals(listOf("A (conflict laptop).md"), plan.conflicts)
        // Without edits, the pull and the delete apply.
        val clean = SyncEngine.plan(out, mapOf("A.md" to "one", "B.md" to "bee"), emptySet(), emptySet())
        assertEquals(listOf("A.md" to "two"), clean.writes)
        assertEquals(listOf("B.md"), clean.deletes)
    }

    @Test
    fun anUnsavedBaseLosesNoNotes() {
        val bucket = Mem()
        val r = remote(false)
        val laptop = Device("nobase", "laptop")
        val phone = Device("nobase", "phone")
        laptop.write("A.md", "a")
        laptop.sync(bucket, r)
        // The phone stops after the sync and before it applies the outcome.
        assertEquals(1, phone.run(bucket, r).pulls.size)
        val again = phone.sync(bucket, r)
        assertEquals(listOf("A.md"), again.pulls.map { it.path })
        assertEquals(0, again.deletedRemote)
        assertEquals("a", phone.read("A.md"))
    }

    @Test
    fun skipsPathsThatMustNotSync() {
        val bucket = Mem()
        val r = remote(false)
        val d = Device("skip", "laptop")
        d.write("A.md", "a")
        d.sync(bucket, r)
        // A manifest entry that points outside the folder never comes down.
        val m = Json.parse(String(bucket.objects.getValue(".quire/manifest.json"))) as Map<*, *>
        val files = LinkedHashMap(m["files"] as Map<*, *>)
        files["../evil.md"] = mapOf("hash" to "x", "object" to "evil", "time" to 1L, "device" to "x")
        bucket.objects[".quire/manifest.json"] = Json.write(mapOf("version" to 1, "files" to files)).toByteArray()
        bucket.objects["evil"] = "bad".toByteArray()
        val out = Device("skip2", "phone").sync(bucket, r)
        assertEquals(listOf("A.md"), out.pulls.map { it.path })
        assertFalse(File(d.root.parentFile, "evil.md").exists())
    }

    @Test
    fun aFolderThatGetsANoteBeforeTheDeleteLandsStays() {
        val bucket = Mem()
        val r = remote(false)
        val laptop = Device("dirguard", "laptop")
        val phone = Device("dirguard", "phone")
        laptop.mkdir("Old")
        laptop.sync(bucket, r)
        phone.sync(bucket, r)
        laptop.rmdir("Old")
        laptop.sync(bucket, r)
        val out = phone.run(bucket, r)
        assertEquals(listOf("Old"), out.rmdirs)
        // The person adds a note to the folder before the outcome lands.
        assertEquals(emptyList<String>(), SyncEngine.plan(out, mapOf("Old/Fresh.md" to "x"), setOf("Old"), emptySet()).rmdirs)
        assertEquals(listOf("Old"), SyncEngine.plan(out, emptyMap(), setOf("Old"), emptySet()).rmdirs)
    }

    @Test
    fun readsTheOldBaseForm() {
        val f = File(Files.createTempDirectory("quire-base").toFile(), "b.json")
        f.writeText("""{"A.md":"aa","B/C.md":"cc"}""")
        assertEquals(Base(mapOf("A.md" to "aa", "B/C.md" to "cc")), SyncEngine.loadBase(f))
        SyncEngine.saveBase(f, Base(mapOf("A.md" to "aa"), setOf("B", "A")))
        assertEquals("""{"files":{"A.md":"aa"},"folders":["A","B"],"stamps":{}}""", f.readText())
        assertEquals(Base(mapOf("A.md" to "aa"), setOf("A", "B")), SyncEngine.loadBase(f))
        // The stamps take the form of the desktop record: size, time and hash.
        val b = Base(mapOf("a.png" to "hh"), emptySet(), mapOf("a.png" to ImgStamp(5000, 1762941600000, "hh")))
        SyncEngine.saveBase(f, b)
        assertEquals("""{"files":{"a.png":"hh"},"folders":[],"stamps":{"a.png":[5000,1762941600000,"hh"]}}""", f.readText())
        assertEquals(b, SyncEngine.loadBase(f))
        assertEquals(Base(), SyncEngine.loadBase(File(f.parentFile, "missing.json")))
    }

    /** Images sync beside the notes: plain and encrypted, edits and deletes carry over, and a change on both sides keeps both images. */
    @Test
    fun imagesSyncWithTheNotes() {
        for (encrypt in listOf(false, true)) {
            val bucket = Mem()
            val r = remote(encrypt)
            val tag = "img-$encrypt"
            val laptop = Device(tag, "laptop")
            val phone = Device(tag, "phone")
            val png = ByteArray(5000) { (it % 256).toByte() }
            File(laptop.root, "attachments").mkdirs()
            File(laptop.root, "attachments/cat.png").writeBytes(png)
            File(laptop.root, "attachments/notes.txt").writeText("not synced")
            laptop.write("Pics.md", "![cat](attachments/cat.png)")
            assertEquals("the note and the image, not the text file", 2, laptop.sync(bucket, r).uploaded)
            val out = phone.sync(bucket, r)
            assertEquals(1, out.imagePulls.size)
            assertTrue(png.contentEquals(phone.bytes("attachments/cat.png")))
            // A quiet sync reads no image again and changes nothing.
            laptop.reads = 0
            val quiet = laptop.sync(bucket, r)
            assertEquals(0 to 0, quiet.uploaded to quiet.imagePulls.size)
            assertEquals("the stamp saves the read", 0, laptop.reads)
            // An edit on the phone reaches the laptop.
            Thread.sleep(20)
            File(phone.root, "attachments/cat.png").writeBytes("new pixels".toByteArray())
            phone.sync(bucket, r)
            laptop.sync(bucket, r)
            assertEquals("new pixels", String(laptop.bytes("attachments/cat.png")!!))
            // Both change it: each keeps its own, and the other comes in beside it.
            Thread.sleep(20)
            File(laptop.root, "attachments/cat.png").writeBytes("laptop pixels".toByteArray())
            File(phone.root, "attachments/cat.png").writeBytes("phone pixels".toByteArray())
            laptop.sync(bucket, r)
            val both = phone.sync(bucket, r)
            assertEquals(listOf("attachments/cat (conflict laptop).png"), both.conflicts)
            assertEquals("laptop pixels", String(phone.bytes("attachments/cat (conflict laptop).png")!!))
            assertEquals("phone pixels", String(phone.bytes("attachments/cat.png")!!))
            // The laptop gets both versions too.
            laptop.sync(bucket, r)
            assertEquals("phone pixels", String(laptop.bytes("attachments/cat.png")!!))
            assertEquals("laptop pixels", String(laptop.bytes("attachments/cat (conflict laptop).png")!!))
            // A delete on the phone removes it on the laptop, into the trash with its extension.
            assertTrue(File(phone.root, "attachments/cat (conflict laptop).png").delete())
            phone.sync(bucket, r)
            laptop.sync(bucket, r)
            laptop.sync(bucket, r)
            assertNull(laptop.bytes("attachments/cat (conflict laptop).png"))
            assertEquals("laptop pixels", String(laptop.bytes(".trash/attachments - cat (conflict laptop).png")!!))
            // The attachments folder stays, because an image is kept in it.
            assertTrue(laptop.isDir("attachments") && phone.isDir("attachments"))
            if (encrypt) {
                assertTrue("image names are hidden: ${bucket.objects.keys}", bucket.objects.keys.none { it.contains("cat") })
            } else {
                assertTrue(bucket.objects.keys.toString(), "attachments/cat.png" in bucket.objects.keys)
            }
        }
    }

    @Test
    fun anImageThatChangedDuringTheSyncKeepsItsBytes() {
        val bucket = Mem()
        val r = remote(false)
        val laptop = Device("imgguard", "laptop")
        val phone = Device("imgguard", "phone")
        File(laptop.root, "a.png").writeBytes("one".toByteArray())
        File(laptop.root, "b.png").writeBytes("bee".toByteArray())
        laptop.sync(bucket, r)
        phone.sync(bucket, r)
        Thread.sleep(20)
        File(laptop.root, "a.png").writeBytes("two".toByteArray())
        assertTrue(File(laptop.root, "b.png").delete())
        laptop.sync(bucket, r)
        val out = phone.run(bucket, r)
        assertEquals(listOf("a.png"), out.imagePulls.map { it.path })
        assertEquals(listOf("b.png"), out.imageRemoves.map { it.first })
        // The person changes both images before the outcome lands.
        val now = mapOf("a.png" to Sign.sha256Hex("mine"), "b.png" to Sign.sha256Hex("mine too"))
        val plan = SyncEngine.planImages(out, now.keys) { now[it] }
        assertEquals(listOf("a (conflict laptop).png"), plan.writes.map { it.first })
        assertEquals(emptyList<String>(), plan.deletes)
        val clean = SyncEngine.planImages(out, setOf("a.png", "b.png")) { p -> Sign.sha256Hex(if (p == "a.png") "one" else "bee") }
        assertEquals(listOf("a.png"), clean.writes.map { it.first })
        assertEquals(listOf("b.png"), clean.deletes)
    }

    @Test
    fun pinMergeRules() {
        val l = setOf("a", "b")
        val r = setOf("b", "c")
        val b = setOf("b")
        // Added on either side: kept.
        assertEquals(setOf("a", "b", "c"), SyncEngine.mergePins(l, r, b))
        // Removed on one side: removed.
        assertEquals(setOf("a"), SyncEngine.mergePins(setOf("a"), setOf("a", "b"), setOf("a", "b")))
        assertEquals(setOf("a"), SyncEngine.mergePins(setOf("a", "b"), setOf("a"), setOf("a", "b")))
        // A manifest from a version without pins: the pins here stay.
        assertEquals(l, SyncEngine.mergePins(l, null, b))
        // The first sync with pins on this device removes nothing.
        assertEquals(setOf("a", "b", "c"), SyncEngine.mergePins(l, r, null))
    }

    @Test
    fun pinsSync() {
        val bucket = Mem()
        val r = remote(true)
        val laptop = Device("pins", "laptop")
        val phone = Device("pins", "phone")
        for (n in listOf("A.md", "B.md", "Drafts/C.md")) laptop.write(n, "x")
        var lp = laptop.syncPins(bucket, r, setOf("A.md", "B.md"))
        var pp = phone.syncPins(bucket, r, emptySet())
        assertEquals("the phone gets the pins of the laptop", setOf("A.md", "B.md"), pp)
        // The phone unpins A and pins C. The laptop gets both changes.
        pp = phone.syncPins(bucket, r, setOf("B.md", "Drafts/C.md"))
        lp = laptop.syncPins(bucket, r, lp)
        assertEquals(setOf("B.md", "Drafts/C.md"), lp)
        // A rename on the laptop moves the pin, on the phone too.
        assertTrue(File(laptop.root, "B.md").renameTo(File(laptop.root, "D.md")))
        lp = laptop.syncPins(bucket, r, setOf("D.md", "Drafts/C.md"))
        pp = phone.syncPins(bucket, r, pp)
        assertEquals(setOf("D.md", "Drafts/C.md"), pp)
        assertEquals("x", phone.read("D.md"))
        // A deleted note loses its pin on both devices.
        assertTrue(File(phone.root, "Drafts/C.md").delete())
        pp = phone.syncPins(bucket, r, pp)
        assertEquals(setOf("D.md"), pp)
        lp = laptop.syncPins(bucket, r, lp)
        assertEquals(setOf("D.md"), lp)
        // A sync with no pin change writes no pins.
        val before = bucket.objects.mapValues { it.value.toList() }
        laptop.syncPins(bucket, r, lp)
        assertEquals(before, bucket.objects.mapValues { it.value.toList() })
    }

    @Test
    fun aManifestWithoutPinsGetsThemOnlyWhenThereArePins() {
        val bucket = Mem()
        val r = remote(false)
        val d = Device("nopins", "laptop")
        d.write("A.md", "a")
        d.syncPins(bucket, r, emptySet())
        assertFalse("no pins, no key", String(bucket.objects.getValue(".quire/manifest.json")).contains("\"pins\""))
        assertEquals(setOf("A.md"), d.syncPins(bucket, r, setOf("A.md")))
        assertTrue(String(bucket.objects.getValue(".quire/manifest.json")).contains("\"pins\":[\"A.md\"]"))
        // The base keeps the pins after the sync.
        assertEquals(setOf("A.md"), d.syncPins(bucket, r, setOf("A.md")))
    }

    @Test
    fun testReportsWhatTheStoreHolds() {
        val bucket = Mem()
        val r = remote(false)
        assertEquals("Connected to mem. 3 notes will upload on the first sync.", SyncEngine.test(r, 3, false, bucket))
        assertEquals("Connected to mem. Everything is up to date.", SyncEngine.test(r, 3, true, bucket))
        // Test connection only reads.
        assertTrue(bucket.objects.isEmpty())
        Device("test", "laptop").also { it.write("A.md", "a") }.sync(bucket, r)
        assertEquals("Connected to mem. It holds 1 note.", SyncEngine.test(r, 0, true, bucket))
        assertEquals(
            "Connected to mem. The bucket holds 1 note, and this folder has 4 notes. The first sync brings them together.",
            SyncEngine.test(r, 4, false, bucket),
        )
        // Images count apart from the notes, and the message names the remote folder.
        val pics = Mem()
        val inFolder = Remote("mem", emptyMap(), "quire/Field Notes/", false, "")
        val d = Device("test2", "laptop")
        File(d.root, "cat.png").writeBytes(byteArrayOf(1, 2))
        d.write("A.md", "a")
        d.sync(pics, inFolder)
        assertEquals(
            "Connected to mem. The remote folder quire/Field Notes holds 1 note and 1 image, and this folder has 3 notes. The first sync brings them together.",
            SyncEngine.test(inFolder, 3, false, pics),
        )
    }
}
