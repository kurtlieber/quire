package app.quire.notes.state

import android.app.Application
import android.content.ClipboardManager
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.os.Build
import android.os.SystemClock
import android.view.KeyEvent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.core.content.FileProvider
import androidx.documentfile.provider.DocumentFile
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import app.quire.notes.BuildConfig
import app.quire.notes.core.Fmt
import app.quire.notes.core.Format
import app.quire.notes.core.ImageSource
import app.quire.notes.core.Images
import app.quire.notes.core.Img
import app.quire.notes.core.Lock
import app.quire.notes.core.LockKey
import app.quire.notes.core.Md
import app.quire.notes.core.Pins
import app.quire.notes.core.Release
import app.quire.notes.core.Update
import app.quire.notes.core.Sample
import app.quire.notes.core.Sync
import app.quire.notes.core.Themes
import app.quire.notes.core.sync.LocalImage
import app.quire.notes.core.sync.Outcome
import app.quire.notes.core.sync.Sign
import app.quire.notes.core.sync.Remote
import app.quire.notes.core.sync.SyncEngine
import app.quire.notes.core.sync.SyncException
import app.quire.notes.data.Loaded
import app.quire.notes.data.Stamp
import app.quire.notes.data.LocalFs
import app.quire.notes.data.SafFs
import app.quire.notes.data.SecretStore
import app.quire.notes.data.Session
import app.quire.notes.data.Settings
import app.quire.notes.data.Store
import app.quire.notes.data.SyncSettings
import app.quire.notes.data.VaultFs
import app.quire.notes.data.VaultKind
import app.quire.notes.data.VaultRef
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.max
import kotlin.math.min

enum class Screen { Loading, Welcome, Vault }
enum class Mode { Source, Preview }
enum class Panel { Files, Search, Tags }

data class PaletteState(val mode: String, val q: String = "", val idx: Int = 0)
/** The sync message in the settings. [state] is "idle", "testing", "ok" or "error". */
data class SyncStatus(val state: String = "idle", val msg: String = "", val missing: List<String> = emptyList())
data class ScrollReq(val line: Int, val token: Long = System.nanoTime())
data class OpenSel(
    val start: Int,
    val end: Int,
    val line: Int? = null,
    val forceSource: Boolean = false,
    val hit: Boolean = false,
    val focus: Boolean = false,
)

data class Seg(val t: String, val hit: Boolean)
class PalItem(val segs: List<Seg>, val sub: String, val run: () -> Unit)
class PalData(val mode: String, val items: List<PalItem>, val idx: Int)
class Cmd(val label: String, val run: () -> Unit)

private const val SAMPLE_ID = "fieldnotes"

class QuireViewModel(app: Application) : AndroidViewModel(app) {
    private val ctx = app
    private val store = Store(app)
    private val secrets = SecretStore(app)

    // One background thread keeps disk writes in order. It outlives the view model so pending writes finish.
    private val io = Dispatchers.IO.limitedParallelism(1)
    private val ioScope = CoroutineScope(SupervisorJob() + io)

    var settings by mutableStateOf(store.loadSettings()); private set
    var vaults by mutableStateOf(store.loadVaults()); private set
    var screen by mutableStateOf(Screen.Loading); private set
    var vault by mutableStateOf<VaultRef?>(null); private set
    private var fs: VaultFs? = null

    var files by mutableStateOf<Map<String, String>>(emptyMap()); private set
    var folders by mutableStateOf<Set<String>>(emptySet()); private set
    var names by mutableStateOf<Map<String, String>>(emptyMap()); private set
    var tabs by mutableStateOf<List<String>>(emptyList()); private set
    var active by mutableStateOf<String?>(null); private set
    var recent by mutableStateOf<List<String>>(emptyList()); private set
    var expanded by mutableStateOf<Set<String>>(emptySet()); private set
    var mode by mutableStateOf(if (store.mode == "preview") Mode.Preview else Mode.Source); private set

    var leftPanel by mutableStateOf(Panel.Files)
    var query by mutableStateOf("")
    private var drawerOpenState by mutableStateOf(false)

    /** Closing the drawer also drops the new-folder field. */
    var drawerOpen: Boolean
        get() = drawerOpenState
        set(v) {
            drawerOpenState = v
            if (!v) newFolderParent = null
        }
    var outlineOpen by mutableStateOf(false)
    var vaultMenu by mutableStateOf(false)
    var palette by mutableStateOf<PaletteState?>(null)
    var settingsOpen by mutableStateOf(false)
    var setSec by mutableStateOf("appearance")
    var focus by mutableStateOf(false); private set
    var renaming by mutableStateOf(false); private set
    var renameVal by mutableStateOf(TextFieldValue(""))
    var creating by mutableStateOf<String?>(null)

    /** The text and selection of the active note in source mode. */
    var editor by mutableStateOf(TextFieldValue("")); private set

    /** Changes each time the editor loads another note. */
    var docKey by mutableLongStateOf(0L); private set

    /** A search match to mark in the editor until the next edit or tap. */
    var hit by mutableStateOf<TextRange?>(null); private set
    var scrollReq by mutableStateOf<ScrollReq?>(null)
    var pendingEditorFocus by mutableStateOf(false)
    /** True when the search field must take the focus the next time it shows. */
    var searchFocusPending by mutableStateOf(false)
    var prevHead by mutableIntStateOf(0)
    var syncStatus by mutableStateOf(SyncStatus()); private set

    /** A sync runs now. */
    var syncBusy by mutableStateOf(false); private set

    /** Test connection runs now. */
    var syncTesting by mutableStateOf(false); private set

    /** The last sync failed with this message. */
    var syncError by mutableStateOf<String?>(null); private set

    /** The wall clock for "synced 2m ago". It moves every 30 seconds while the app is in front. */
    var syncClock by mutableLongStateOf(System.currentTimeMillis()); private set
    var reveal by mutableStateOf<Set<String>>(emptySet())
    var editorFocused by mutableStateOf(false)
    var systemDark by mutableStateOf(false)

    /** A short message for the status bar or the welcome screen, for example a failed save. */
    var notice by mutableStateOf<String?>(null)
    private var noticeJob: Job? = null

    /** The folder that gets a new subfolder from the inline name field. Null hides the field. "" is the root. */
    var newFolderParent by mutableStateOf<String?>(null); private set
    var newFolderName by mutableStateOf(TextFieldValue(""))

    private val dirty = LinkedHashSet<String>()

    // Notes that move on disk right now. Their edits wait until the move is done.
    private val moving = HashSet<String>()
    private var saveJob: Job? = null
    private var sessionJob: Job? = null
    private var resumedOnce = false

    private var syncJob: Job? = null
    private var syncTimer: Job? = null
    private var tickJob: Job? = null
    // When the next sync is due, on the elapsed-time clock. Null when none is due.
    private var syncDue: Long? = null
    // When the last sync started. "Every 5 min" counts from it, so a failed sync does not start again at once.
    private var lastTry: Long? = null
    // Another sync was asked for while one ran.
    private var syncAgain = false
    // The person tapped Sync now, so the settings show how it went.
    private var syncManual = false

    init {
        // The settings load without the old shared remote folder. Saving them once removes it from storage too.
        store.saveSettings(settings)
        startUpdateChecks()
        viewModelScope.launch {
            val stored = withContext(io) { secrets.load() }
            if (stored.isNotEmpty()) settings = mergeSecrets(settings, stored)
            if (!store.sampleCreated) {
                withContext(io) { writeSample(sampleDir()) }
                store.sampleCreated = true
                putVault(sampleRef(Sample.files.size), front = true)
                store.saveSession(SAMPLE_ID, sampleSession())
                store.lastVault = SAMPLE_ID
                store.lastScreen = "vault"
            }
            val last = vaults.find { it.id == store.lastVault }
            if (last != null && store.lastScreen == "vault") openVault(last) else screen = Screen.Welcome
        }
    }

    // ---- Vaults ----------------------------------------------------------------------------

    private fun vaultsDir() = File(ctx.filesDir, "vaults")
    private fun sampleDir() = File(vaultsDir(), Sample.NAME)
    private fun sampleRef(count: Int) = VaultRef(SAMPLE_ID, Sample.NAME, VaultKind.LOCAL, sampleDir().absolutePath, sampleDir().absolutePath, count)
    private fun sampleSession() = Session(Sample.tabs, Sample.tabs[0], Sample.tabs, Sample.expanded)

    private fun writeSample(dir: File) {
        dir.mkdirs()
        Sample.folders.forEach { File(dir, it).mkdirs() }
        Sample.files.forEach { (p, t) -> File(dir, p).also { it.parentFile?.mkdirs() }.writeText(t) }
    }

    private fun fsFor(ref: VaultRef): VaultFs =
        if (ref.kind == VaultKind.SAF) SafFs(ctx, Uri.parse(ref.location)) else LocalFs(File(ref.location))

    private fun putVault(ref: VaultRef, front: Boolean = false) {
        val i = vaults.indexOfFirst { it.id == ref.id }
        vaults = when {
            i >= 0 -> vaults.toMutableList().also { it[i] = ref }
            front -> listOf(ref) + vaults
            else -> vaults + ref
        }
        store.saveVaults(vaults)
    }

    fun shortPath(ref: VaultRef): String =
        if (ref.kind == VaultKind.LOCAL && ref.location.startsWith(vaultsDir().absolutePath)) {
            "~/vaults/" + File(ref.location).name
        } else {
            ref.path
        }

    fun newVaultPath(name: String): String = "~/vaults/" + (Md.safeName(name).ifEmpty { "Untitled notebook" })

    private fun leaveVault() {
        flushAll()
        saveSession()
    }

    fun openVault(ref: VaultRef, session: Session? = null) {
        viewModelScope.launch {
            leaveVault()
            dirty.clear()
            val f = fsFor(ref)
            val loaded = try {
                withContext(io) { f.load().also { f.displayPath } }
            } catch (e: Exception) {
                fs = null
                vault = null
                screen = Screen.Welcome
                store.lastScreen = "welcome"
                flash("Quire can't open ${ref.name}. ${e.message ?: ""}".trim())
                return@launch
            }
            fs = f
            vault = ref.copy(path = f.displayPath, count = loaded.files.size)
            putVault(vault!!)
            setStructure(loaded.files, loaded.folders)
            images = loaded.images
            val ses = session ?: store.loadSession(ref.id)
            tabs = ses.tabs.filter { it in files }
            active = ses.active?.takeIf { it in files } ?: tabs.firstOrNull()
            recent = ses.recent.filter { it in files }.ifEmpty { tabs }
            expanded = ses.expanded
            // Pins of files that are gone drop out when the folder loads.
            pinned = Pins.filter(ses.pinned, loaded.files.keys)
            pinsOpen = ses.pinsOpen
            query = ""
            leftPanel = Panel.Files
            focus = false
            creating = null
            palette = null
            settingsOpen = false
            renaming = false
            drawerOpen = false
            outlineOpen = false
            loadEditor(active, null)
            screen = Screen.Vault
            store.lastVault = ref.id
            store.lastScreen = "vault"
            saveSession()
            syncError = null
            syncAuto(500)
        }
    }

    fun createVault() {
        val name = Md.safeName(creating ?: "").ifEmpty { "Untitled notebook" }
        viewModelScope.launch {
            val dir = withContext(io) {
                var d = File(vaultsDir(), name)
                var n = 2
                while (d.exists()) d = File(vaultsDir(), "$name ${n++}")
                d.mkdirs()
                d
            }
            val ref = VaultRef("v" + System.currentTimeMillis(), dir.name, VaultKind.LOCAL, dir.absolutePath, dir.absolutePath, 0)
            putVault(ref)
            creating = null
            openVault(ref)
        }
    }

    /** Opens a folder from ACTION_OPEN_DOCUMENT_TREE. The caller has taken the persistable permission. */
    fun openTree(uri: Uri) {
        val existing = vaults.find { it.kind == VaultKind.SAF && it.location == uri.toString() }
        if (existing != null) {
            openVault(existing)
            return
        }
        viewModelScope.launch {
            val (name, path) = withContext(io) {
                val n = try {
                    DocumentFile.fromTreeUri(ctx, uri)?.name
                } catch (e: Exception) {
                    null
                }
                (n ?: "Folder") to SafFs.describe(ctx, uri)
            }
            val ref = VaultRef("v" + System.currentTimeMillis(), name, VaultKind.SAF, uri.toString(), path, 0)
            putVault(ref)
            openVault(ref)
        }
    }

    /** Takes a vault off the recent list. The folder and its notes stay on disk. */
    fun forgetVault(ref: VaultRef) {
        vaults = vaults.filter { it.id != ref.id }
        store.saveVaults(vaults)
        store.forgetSession(ref.id)
        if (ref.kind == VaultKind.SAF) {
            try {
                ctx.contentResolver.releasePersistableUriPermission(
                    Uri.parse(ref.location),
                    android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                )
            } catch (e: Exception) {
                // The permission is already gone.
            }
        }
    }

    fun goWelcome() {
        leaveVault()
        screen = Screen.Welcome
        palette = null
        settingsOpen = false
        vaultMenu = false
        drawerOpen = false
        outlineOpen = false
        focus = false
        renaming = false
        store.lastScreen = "welcome"
    }

    fun resetSample() {
        viewModelScope.launch {
            leaveVault()
            if (vault?.id == SAMPLE_ID) {
                vault = null
                fs = null
            }
            withContext(io) {
                sampleDir().deleteRecursively()
                writeSample(sampleDir())
            }
            putVault(sampleRef(Sample.files.size), front = vaults.none { it.id == SAMPLE_ID })
            store.saveSession(SAMPLE_ID, sampleSession())
            settingsOpen = false
            palette = null
            focus = false
            openVault(sampleRef(Sample.files.size), sampleSession())
        }
    }

    fun onResume() {
        startTicker()
        updateOnResume()
        if (!resumedOnce) {
            resumedOnce = true
            return
        }
        refresh()
        syncAuto(500)
    }

    /** The app goes to the background. Unsaved notes go to disk, and the folder syncs unless sync is manual. */
    fun onPause() {
        tickJob?.cancel()
        tickJob = null
        val auto = syncReady() && settings.sync.whenSync != "manual"
        flushAll(quiet = auto)
        saveSession()
        if (auto) startSync()
    }

    /** Reads the vault again, so edits from other apps show up. Unsaved edits win. */
    private fun refresh() {
        val f = fs ?: return
        val v = vault ?: return
        if (screen != Screen.Vault) return
        viewModelScope.launch {
            val loaded = try {
                withContext(io) { f.load() }
            } catch (e: Exception) {
                return@launch
            }
            if (vault?.id != v.id) return@launch
            if (loaded.images != images) images = loaded.images
            val merged = LinkedHashMap(loaded.files)
            for (p in dirty) files[p]?.let { merged[p] = it }
            if (merged == files && loaded.folders == folders) return@launch
            // An unlocked note that changed on disk opens with the key in memory, or locks again.
            for (p in unlocked.keys.toList()) {
                val t = merged[p]
                if (t != null && t != files[p] && p !in dirty) outsideLockedText(p, t)
            }
            setStructure(merged, loaded.folders)
            tabs = tabs.filter { it in merged }
            recent = recent.filter { it in merged }
            pinned = Pins.filter(pinned, merged.keys)
            val a = active
            if (a == null || a !in merged) {
                active = tabs.firstOrNull()
                loadEditor(active, null)
            } else if (a !in unlocked && editor.text != textOf(a)) {
                val t = textOf(a)
                editor = TextFieldValue(t, shiftSelection(editor, t))
            }
            updateCount()
        }
    }

    private fun updateCount() {
        val v = vault ?: return
        if (v.count != files.size) {
            vault = v.copy(count = files.size)
            putVault(vault!!)
        }
    }

    // ---- Files -----------------------------------------------------------------------------

    private fun setStructure(f: Map<String, String>, dirs: Set<String> = folders) {
        files = f
        folders = dirs
        names = Md.buildNames(f.keys)
    }

    fun folderList(): List<String> = Md.folderList(folders, files.keys)

    private fun setDoc(path: String, text: String) {
        val u = unlocked[path]
        if (u != null) {
            // An unlocked note keeps its plain text in memory. The folder copy is sealed again.
            if (u.text == text) return
            unlocked = unlocked + (path to Unlocked(u.key, text))
            files = files + (path to Lock.seal(u.key, text))
        } else {
            if (noteLocked(path)) return
            files = files + (path to text)
        }
        dirty.add(path)
        saveJob?.cancel()
        saveJob = viewModelScope.launch {
            delay(500)
            flushAll()
        }
    }

    /**
     * Writes every unsaved note on the background thread. After the writes, "On save" starts a
     * sync. With [quiet], no sync starts, because the caller starts one.
     */
    fun flushAll(quiet: Boolean = false) {
        saveJob?.cancel()
        val f = fs ?: return
        if (dirty.isEmpty()) return
        val batch = dirty.filter { it !in moving }.mapNotNull { p -> files[p]?.let { p to it } }
        dirty.removeAll { it !in moving }
        if (batch.isEmpty()) return
        ioScope.launch {
            for ((p, t) in batch) {
                try {
                    f.write(p, t)
                } catch (e: Exception) {
                    withContext(Dispatchers.Main) { flash("Couldn't save ${Md.base(p)}. ${e.message ?: ""}".trim()) }
                }
            }
            if (!quiet) withContext(Dispatchers.Main) { syncAfterSave() }
        }
    }

    private fun disk(what: String, op: VaultFs.() -> Unit) {
        val f = fs ?: return
        ioScope.launch {
            try {
                f.op()
                withContext(Dispatchers.Main) { syncAfterSave() }
            } catch (e: Exception) {
                withContext(Dispatchers.Main) { flash("Couldn't $what. ${e.message ?: ""}".trim()) }
            }
        }
    }

    fun flash(msg: String) {
        notice = msg
        noticeJob?.cancel()
        noticeJob = viewModelScope.launch {
            delay(6000)
            notice = null
        }
    }

    private fun persistSession() {
        sessionJob?.cancel()
        sessionJob = viewModelScope.launch {
            delay(300)
            saveSession()
        }
    }

    private fun saveSession() {
        val v = vault ?: return
        store.saveSession(v.id, Session(tabs, active, recent, expanded, pinned, pinsOpen))
    }

    private fun loadEditor(path: String?, sel: OpenSel?) {
        val t = path?.let { textOf(it) } ?: ""
        val range = if (sel != null) TextRange(sel.start.coerceIn(0, t.length), sel.end.coerceIn(0, t.length)) else TextRange(0)
        editor = TextFieldValue(t, range)
        docKey++
        hit = if (sel?.hit == true && range.length > 0) range else null
        scrollReq = sel?.line?.let { ScrollReq(it) }
        if (sel?.focus == true) pendingEditorFocus = true
    }

    private fun ancestors(path: String): Set<String> {
        val out = HashSet<String>()
        var d = Md.dir(path)
        while (d.isNotEmpty()) {
            out.add(d)
            d = Md.dir(d)
        }
        return out
    }

    fun openFile(path: String, sel: OpenSel? = null) {
        if (path !in files) return
        if (path !in tabs) tabs = tabs + path
        expanded = expanded + ancestors(path)
        recent = (listOf(path) + recent.filter { it != path }).take(30)
        palette = null
        renaming = false
        vaultMenu = false
        drawerOpen = false
        outlineOpen = false
        if (sel?.forceSource == true) setModeInternal(Mode.Source)
        prevHead = 0
        if (sel != null || path != active) {
            active = path
            loadEditor(path, sel)
        }
        persistSession()
    }

    fun closeTab(p: String) {
        // A closed note locks again.
        if (p in unlocked) {
            flushAll()
            unlocked = unlocked - p
        }
        val i = tabs.indexOf(p)
        val nt = tabs.filter { it != p }
        tabs = nt
        if (active == p) {
            val na = if (nt.isEmpty()) null else nt[min(max(i, 0), nt.size - 1)]
            active = na
            loadEditor(na, null)
        }
        renaming = false
        if (active == null) focus = false
        persistSession()
    }

    fun toggleFolder(path: String) {
        expanded = if (path in expanded) expanded - path else expanded + path
        persistSession()
    }

    /**
     * Creates a note. [inFolder] puts it in that folder and makes the folder when it is missing.
     * Without it, the note goes in the folder from settings, like the prototype.
     */
    fun newNote(name: String? = null, inFolder: String? = null) {
        val f = fs ?: return
        var folder = inFolder ?: settings.newFolder
        if (inFolder == null && folder.isNotEmpty() && folder !in folderList()) folder = ""
        val named = !name.isNullOrEmpty()
        val bn = Md.safeName(if (named) name else "Untitled").ifEmpty { "Untitled" }
        var p = Md.join(folder, "$bn.md")
        var n = 2
        while (p in files) p = Md.join(folder, "$bn ${n++}.md")
        val content = if (named) "# $bn\n\n" else ""
        setStructure(files + (p to content), folders + ancestors(p))
        val path = p
        disk("create $bn") { write(path, content) }
        screen = Screen.Vault
        settingsOpen = false
        openFile(p, OpenSel(content.length, content.length, focus = named && mode == Mode.Source))
        if (!named) {
            renaming = true
            renameVal = TextFieldValue(bn, TextRange(0, bn.length))
            setModeInternal(Mode.Source)
            focus = false
        }
        updateCount()
    }

    /** Creates a note from a typed path such as "Drafts/Idea". Missing folders are made too. */
    fun newNoteAtPath(typed: String) {
        val parts = typed.split('/').map { Md.safeName(it) }.filter { it.isNotEmpty() }
        if (parts.isEmpty()) return
        val name = parts.last().replace(Regex("""\.md$""", RegexOption.IGNORE_CASE), "").ifEmpty { return }
        newNote(name, parts.dropLast(1).joinToString("/"))
    }

    private fun where(dir: String) = if (dir.isEmpty()) "the folder root" else "$dir/"

    // ---- Folders -------------------------------------------------------------------------

    /** Shows the inline name field for a new folder in [parent]. "" is the root. */
    fun startNewFolder(parent: String) {
        if (parent.isNotEmpty()) expanded = expanded + parent + ancestors(parent)
        newFolderName = TextFieldValue("")
        newFolderParent = parent
        leftPanel = Panel.Files
        palette = null
        settingsOpen = false
        drawerOpen = true
    }

    fun cancelNewFolder() {
        newFolderParent = null
    }

    /** Makes the folder from the inline field as a real, empty directory. */
    fun commitNewFolder() {
        val parent = newFolderParent ?: return
        newFolderParent = null
        val name = Md.safeName(newFolderName.text)
        if (name.isEmpty()) return
        if (name.startsWith(".")) {
            flash("Quire hides folders that start with a dot. Pick another name.")
            return
        }
        val path = Md.join(parent, name)
        if (folderList().any { it.equals(path, ignoreCase = true) }) {
            flash("A folder named $name is already in ${where(parent)}.")
            return
        }
        setStructure(files, folders + path)
        expanded = expanded + path
        disk("create the folder $name") { mkdirs(path) }
        persistSession()
    }

    // ---- Moving notes --------------------------------------------------------------------

    /** The number of notes in the folder [dir] and its subfolders. */
    fun notesIn(dir: String): Int = files.keys.count { Md.inDir(it, dir) }

    /**
     * Moves the folder [dir] and everything in it to .trash. Unsaved notes in the folder go to
     * disk first, so the trash holds their last text. The tabs of the notes close.
     */
    fun deleteFolder(dir: String) {
        if (dir.isEmpty() || fs == null) return
        flushAll()
        val gone = files.keys.filter { Md.inDir(it, dir) }.toSet()
        dirty.removeAll(gone)
        unlocked = unlocked - gone
        pinned = Pins.removeIn(pinned, dir)
        setStructure(files - gone)
        dropFolder(dir)
        val i = active?.let { tabs.indexOf(it) } ?: -1
        tabs = tabs.filter { it !in gone }
        recent = recent.filter { it !in gone }
        if (active in gone) {
            active = if (tabs.isEmpty()) null else tabs[min(max(i, 0), tabs.size - 1)]
            loadEditor(active, null)
            renaming = false
            if (active == null) focus = false
        }
        disk("delete the folder ${dir.substringAfterLast('/')}") { deleteDir(dir) }
        updateCount()
        persistSession()
    }

    /** Forgets the folder [dir] and its subfolders in the tree state. The notes in it must be gone already. */
    private fun dropFolder(dir: String) {
        folders = folders.filter { it != dir && !Md.inDir(it, dir) }.toSet()
        expanded = expanded.filter { it != dir && !Md.inDir(it, dir) }.toSet()
        newFolderParent?.let { if (it == dir || Md.inDir(it, dir)) newFolderParent = null }
    }

    /**
     * Moves a note into [dir] ("" is the root). The note keeps its tab, caret and undo history.
     * Links that spell out the old path change to the new path. Links by name keep working as they are.
     */
    fun moveNote(path: String, dir: String) {
        val f = fs ?: return
        if (path !in files) return
        val np = Md.join(dir, path.substringAfterLast('/'))
        if (np == path) return
        if (files.keys.any { it != path && it.equals(np, ignoreCase = true) }) {
            flash("${Md.base(np)} is already in ${where(dir)}. Quire did not move the note.")
            return
        }
        flushAll()
        moving.add(path)
        viewModelScope.launch {
            try {
                withContext(io) { f.move(path, np) }
            } catch (e: Exception) {
                moving.remove(path)
                flash("Couldn't move ${Md.base(path)}. ${e.message ?: ""}".trim())
                return@launch
            }
            moving.remove(path)
            applyMove(path, np)
            syncAfterSave()
        }
    }

    private fun applyMove(old: String, np: String) {
        if (old !in files) return
        val oldFull = Md.stripExt(old)
        val newFull = Md.stripExt(np)
        // A link by name still resolves after the move. Only a link with the folder in it needs a new path.
        val rewrite = settings.updateLinks && oldFull.contains('/')
        val re = Regex("""\[\[""" + Regex.escape(oldFull) + """(\|[^\]]*)?\]\]""", RegexOption.IGNORE_CASE)
        val nf = LinkedHashMap<String, String>()
        val changed = ArrayList<String>()
        for ((p, t) in files) {
            val t2 = if (rewrite) re.replace(t) { "[[" + newFull + (it.groups[1]?.value ?: "") + "]]" } else t
            val key = if (p == old) np else p
            nf[key] = t2
            if (t2 != t) changed.add(key)
        }
        val wasDirty = dirty.remove(old)
        unlocked[old]?.let { unlocked = unlocked - old + (np to it) }
        setStructure(nf, folders + ancestors(np))
        tabs = tabs.map { if (it == old) np else it }
        recent = recent.map { if (it == old) np else it }
        pinned = Pins.rename(pinned, old, np)
        expanded = expanded + ancestors(np)
        if (active == old) active = np
        // The open note may be the moved one or one whose links changed. The editor must show the new text.
        active?.let { a -> (unlocked[a]?.text ?: nf[a])?.let { t -> if (editor.text != t) editor = TextFieldValue(t, shiftSelection(editor, t)) } }
        if (wasDirty) dirty.add(np)
        dirty.addAll(changed)
        flushAll()
        persistSession()
    }

    /** Keeps the caret on the same text when a rewrite changes the note before or after it. */
    private fun shiftSelection(v: TextFieldValue, t: String): TextRange {
        val a = v.text
        var pre = 0
        while (pre < a.length && pre < t.length && a[pre] == t[pre]) pre++
        var suf = 0
        while (suf < a.length - pre && suf < t.length - pre && a[a.length - 1 - suf] == t[t.length - 1 - suf]) suf++
        fun map(i: Int) = when {
            i <= pre -> i
            i >= a.length - suf -> i + (t.length - a.length)
            else -> pre
        }.coerceIn(0, t.length)
        return TextRange(map(v.selection.start), map(v.selection.end))
    }

    fun deleteNote() {
        val p = active ?: return
        unlocked = unlocked - p
        pinned = Pins.remove(pinned, p)
        dirty.remove(p)
        val i = tabs.indexOf(p)
        val nt = tabs.filter { it != p }
        setStructure(files - p)
        tabs = nt
        recent = recent.filter { it != p }
        focus = false
        val na = if (nt.isEmpty()) null else nt[min(max(i, 0), nt.size - 1)]
        active = na
        loadEditor(na, null)
        disk("delete ${Md.base(p)}") { delete(p) }
        updateCount()
        persistSession()
    }

    fun openToday() {
        val d = LocalDate.now()
        val p = "Journal/$d.md"
        val existing = files[p]
        if (existing == null) {
            val c = "# ${d.format(DateTimeFormatter.ofPattern("EEEE d MMMM", Locale.UK))}\n\n#journal\n\n"
            setStructure(files + (p to c), folders + "Journal")
            disk("create today's entry") { write(p, c) }
            openFile(p, OpenSel(c.length, c.length, forceSource = true, focus = true))
            updateCount()
        } else {
            openFile(p, OpenSel(existing.length, existing.length, forceSource = true, focus = true))
        }
    }

    fun openLink(name: String) {
        val t = name.split('|')[0].trim()
        // An image file is no note, so a link to it never makes one.
        if (t.isEmpty() || Md.isImage(t)) return
        val p = Md.resolve(names, t)
        if (p != null) openFile(p) else newNote(t)
    }

    fun startRename() {
        val a = active ?: return
        val b = Md.base(a)
        renameVal = TextFieldValue(b, TextRange(0, b.length))
        renaming = true
    }

    fun cancelRename() {
        renaming = false
    }

    fun commitRename() {
        if (!renaming) return
        val old = active
        if (old == null) {
            renaming = false
            return
        }
        val v = Md.safeName(renameVal.text)
        val np = Md.join(Md.dir(old), "$v.md")
        if (v.isEmpty() || np == old || np in files) {
            renaming = false
            return
        }
        val oldBase = Md.base(old)
        val nf = LinkedHashMap<String, String>()
        val changed = ArrayList<String>()
        for ((p, t) in files) {
            val t2 = if (settings.updateLinks) Md.rewriteLinks(t, oldBase, v) else t
            val key = if (p == old) np else p
            nf[key] = t2
            if (p != old && t2 != t) changed.add(key)
        }
        val activeText = nf.getValue(np)
        val writeActive = dirty.remove(old) || activeText != files[old]
        // An unlocked note keeps its plain text under the new name. The disk gets the sealed text.
        val u = unlocked[old]
        if (u != null) unlocked = unlocked - old + (np to u)
        val shown = u?.text ?: activeText
        setStructure(nf)
        tabs = tabs.map { if (it == old) np else it }
        recent = recent.map { if (it == old) np else it }
        pinned = Pins.rename(pinned, old, np)
        active = np
        renaming = false
        if (editor.text != shown) {
            editor = TextFieldValue(shown, shiftSelection(editor, shown))
        }
        disk("rename $oldBase") {
            rename(old, np)
            if (writeActive) write(np, activeText)
        }
        dirty.addAll(changed)
        flushAll()
        persistSession()
    }

    // ---- Editor ----------------------------------------------------------------------------

    /**
     * Takes an edit from the editor that shows document [key]. A field for an older document can
     * still send a late selection change after a link opens another note. Quire ignores it, so it
     * can never write one note's text into another.
     */
    fun onEditorChange(key: Long, v: TextFieldValue) {
        if (key != docKey) return
        val p = active ?: return
        val old = editor
        if (v.text != old.text) {
            var nv = v
            if (settings.continueLists && isEnterInsert(old, v)) {
                Md.continueList(old.text, old.selection.start)?.let { (t, caret) -> nv = TextFieldValue(t, TextRange(caret)) }
            }
            editor = nv
            hit = null
            setDoc(p, nv.text)
        } else {
            if (v.selection != old.selection) hit = null
            editor = v
        }
    }

    private fun isEnterInsert(o: TextFieldValue, n: TextFieldValue): Boolean {
        if (!o.selection.collapsed) return false
        val pos = o.selection.start
        if (n.text.length != o.text.length + 1 || n.selection != TextRange(pos + 1)) return false
        if (pos >= n.text.length || n.text[pos] != '\n') return false
        return n.text.regionMatches(0, o.text, 0, pos) && n.text.regionMatches(pos + 1, o.text, pos, o.text.length - pos)
    }

    private fun applyEdit(nv: TextFieldValue) {
        val p = active ?: return
        editor = nv
        hit = null
        setDoc(p, nv.text)
    }

    fun wrapSelection(mk: String) = format(if (mk == "**") "bold" else "italic")

    /** The actions of the Markdown toolbar, in its order. */
    val toolbar = listOf("bold", "italic", "strike", "heading", "link", "noteLink", "bullet", "number", "task", "quote", "code", "image", "pin", "lock")

    /** Runs one action of the Markdown toolbar on the selection of the editor. */
    fun format(action: String) {
        if (action == "image") {
            requestImagePick()
            return
        }
        if (action == "lock") {
            askLock()
            return
        }
        if (action == "pin") {
            active?.let { togglePin(it) }
            return
        }
        val a = active ?: return
        if (noteLocked(a)) return
        val f = when (action) {
            "bold" -> Fmt.BOLD
            "italic" -> Fmt.ITALIC
            "strike" -> Fmt.STRIKE
            "code" -> Fmt.CODE
            "heading" -> Fmt.HEADING
            "link" -> Fmt.LINK
            "noteLink" -> Fmt.WIKI
            "bullet" -> Fmt.BULLET
            "number" -> Fmt.NUMBER
            "task" -> Fmt.TASK
            "quote" -> Fmt.QUOTE
            else -> return
        }
        val v = editor
        val ed = Format.apply(v.text, v.selection.start, v.selection.end, f)
        applyEdit(TextFieldValue(ed.applyTo(v.text), TextRange(ed.selStart, ed.selEnd)))
    }

    fun insertText(s: String) {
        val v = editor
        val a = v.selection.min
        val b = v.selection.max
        applyEdit(TextFieldValue(v.text.substring(0, a) + s + v.text.substring(b), TextRange(a + s.length)))
    }

    fun caretToEnd() {
        editor = editor.copy(selection = TextRange(editor.text.length))
        pendingEditorFocus = true
    }

    fun toggleTask(line: Int) {
        val p = active ?: return
        if (noteLocked(p)) return
        val t = textOf(p)
        val lines = t.split('\n').toMutableList()
        if (line !in lines.indices) return
        lines[line] = Md.toggleTaskLine(lines[line])
        val nt = lines.joinToString("\n")
        if (nt == t) return
        editor = TextFieldValue(nt, TextRange(min(editor.selection.start, nt.length), min(editor.selection.end, nt.length)))
        setDoc(p, nt)
    }

    /** Moves to a heading line. In source mode the caret goes there too. */
    fun jump(line: Int) {
        outlineOpen = false
        if (mode == Mode.Source) {
            val v = editor.text
            var pos = 0
            for (i in 0 until line) {
                val n = v.indexOf('\n', pos)
                if (n < 0) break
                pos = n + 1
            }
            editor = editor.copy(selection = TextRange(pos))
        }
        scrollReq = ScrollReq(line)
    }

    /** The link under the caret, as the status bar hint shows it. */
    fun caretLink(): Pair<IntRange, String>? {
        if (mode != Mode.Source || active == null) return null
        val doc = editor.text
        val pos = editor.selection.start.coerceIn(0, doc.length)
        val ls = if (pos == 0) 0 else doc.lastIndexOf('\n', pos - 1) + 1
        val le = doc.indexOf('\n', pos).let { if (it < 0) doc.length else it }
        val r = Md.linkAt(doc.substring(ls, le), pos - ls) ?: return null
        return IntRange(r.first.first + ls, r.first.last + ls) to r.second
    }

    // ---- Modes and panels ------------------------------------------------------------------

    private fun setModeInternal(m: Mode) {
        mode = m
        store.mode = if (m == Mode.Preview) "preview" else "source"
    }

    fun setModeTo(m: Mode) {
        if (m != mode) toggleMode()
    }

    fun toggleMode(focusEditor: Boolean = false) {
        if (active == null) return
        val toSource = mode == Mode.Preview
        setModeInternal(if (mode == Mode.Source) Mode.Preview else Mode.Source)
        if (toSource && focusEditor) pendingEditorFocus = true
    }

    fun toggleFocus() {
        if (active == null) return
        focus = !focus
        palette = null
        if (focus && mode == Mode.Source) pendingEditorFocus = true
    }

    fun leaveFocus() {
        focus = false
    }

    fun openSearch() {
        searchFocusPending = true
        leftPanel = Panel.Search
        drawerOpen = true
        focus = false
        palette = null
        settingsOpen = false
    }

    fun searchTag(tag: String) {
        leftPanel = Panel.Search
        query = "#$tag"
        drawerOpen = true
        focus = false
        palette = null
        outlineOpen = false
    }

    fun openPalette(m: String) {
        palette = PaletteState(m)
        vaultMenu = false
        drawerOpen = false
        settingsOpen = false
        outlineOpen = false
    }

    fun closePalette() {
        palette = null
    }

    fun openSettings() {
        settingsOpen = true
        vaultMenu = false
        drawerOpen = false
    }

    fun isDark(): Boolean = settings.theme == "dark" || (settings.theme == "system" && systemDark)

    /** The theme picked for dark or light mode. Without a pick it is Ink or Paper. */
    fun themeId(dark: Boolean): String {
        val id = if (dark) settings.darkTheme else settings.lightTheme
        return id?.let { Themes.def(it) }?.takeIf { it.dark == dark }?.id ?: if (dark) "ink" else "paper"
    }

    /** Picks the theme for one mode. A pick in the list of the other mode switches to that mode, unless the mode is Match system. */
    fun pickTheme(id: String, dark: Boolean) = updateSettings { s ->
        val t = if (dark) s.copy(darkTheme = id) else s.copy(lightTheme = id)
        if (s.theme == "system") t else t.copy(theme = if (dark) "dark" else "light")
    }

    /** Sets the accent: "#rrggbb", or empty for the accent of the theme. */
    fun setAccent(hex: String) = updateSettings { it.copy(accent = hex) }

    // ---- Settings --------------------------------------------------------------------------

    fun updateSettings(f: (Settings) -> Settings) {
        val before = settings
        settings = f(settings)
        store.saveSettings(settings)
        if (settings.sync != before.sync) {
            val snap = secretMap(settings)
            if (snap != secretMap(before)) ioScope.launch { secrets.save(snap) }
        }
    }

    private fun secretMap(s: Settings): Map<String, String> {
        val out = HashMap<String, String>()
        for ((pid, m) in s.sync.cfg) for ((k, v) in m) if (k in Sync.secretKeys) out["$pid.$k"] = v
        if (s.sync.pass.isNotEmpty()) out["sync.pass"] = s.sync.pass
        return out
    }

    private fun mergeSecrets(s: Settings, sec: Map<String, String>): Settings {
        val cfg = s.sync.cfg.mapValues { it.value.toMutableMap() }.toMutableMap()
        for ((k, v) in sec) {
            if (k == "sync.pass") continue
            val pid = k.substringBefore('.')
            val key = k.substringAfter('.')
            cfg.getOrPut(pid) { HashMap() }[key] = v
        }
        return s.copy(sync = s.sync.copy(cfg = cfg, pass = sec["sync.pass"] ?: ""))
    }

    private fun setSync(f: (SyncSettings) -> SyncSettings) = updateSettings { it.copy(sync = f(it.sync)) }

    /** The form values of the picked provider. The remote folder comes from the open notes folder. */
    fun syncCfg(): Map<String, String> = (settings.sync.cfg[settings.sync.provider] ?: emptyMap()) + ("prefix" to remoteFolder())

    fun selectProvider(id: String) {
        if (id == settings.sync.provider) return
        setSync { it.copy(provider = id, connected = false) }
        syncStatus = SyncStatus()
    }

    fun setCfg(key: String, v: String) {
        setSync { sy ->
            if (key == "prefix") {
                // The remote folder belongs to the open notes folder.
                val id = vault?.id ?: return@setSync sy.copy(connected = false)
                sy.copy(folders = sy.folders + (id to v), connected = false)
            } else {
                val pid = sy.provider
                val m = (sy.cfg[pid] ?: emptyMap()) + (key to v)
                sy.copy(cfg = sy.cfg + (pid to m), connected = false)
            }
        }
        val st = syncStatus
        syncStatus = if (st.state == "error") st.copy(missing = st.missing - key) else SyncStatus()
    }

    fun setWhen(v: String) = setSync { it.copy(whenSync = v) }
    fun toggleEncrypt() = setSync { it.copy(encrypt = !it.encrypt, connected = false) }
    fun setPass(v: String) = setSync { it.copy(pass = v, connected = false) }

    // ---- Updates ---------------------------------------------------------------------------

    /** The version of this app, for example 0.5.3. */
    val appVersion: String = BuildConfig.VERSION_NAME

    /** A debug build cannot take a release APK, because the signatures differ. So it does not check. */
    val updatesAllowed: Boolean = !BuildConfig.DEBUG

    /** A newer release with an APK, or null. */
    var update by mutableStateOf<Release?>(null); private set

    /** True while the APK of the update downloads. */
    var updating by mutableStateOf(false); private set

    /** True while Check now runs. */
    var checking by mutableStateOf(false); private set

    /** The answer of Check now, or null. */
    var updateNote by mutableStateOf<String?>(null); private set

    // When the last check started, on the elapsed-time clock. 0 before the first check.
    private var lastCheck = 0L
    // A downloaded APK that waits for the permission to install apps.
    private var pendingInstall: File? = null

    private val sixHours = 6L * 60 * 60 * 1000

    /** The Update button shows. */
    fun showUpdate(): Boolean = updatesAllowed && settings.checkUpdates && update != null

    private fun updatesDir() = File(ctx.cacheDir, "updates")

    /** Deletes old APKs, then checks 10 s after the start and every 6 hours while the app runs. */
    private fun startUpdateChecks() {
        updatesDir().listFiles()?.forEach { if (it.name.endsWith(".apk") || it.name.endsWith(".part")) it.delete() }
        if (!updatesAllowed) return
        viewModelScope.launch {
            delay(10_000)
            while (true) {
                if (settings.checkUpdates) checkUpdate(manual = false)
                delay(sixHours)
            }
        }
    }

    /** Back in front: a check when 6 hours have gone by, and the install that waited for its permission. */
    private fun updateOnResume() {
        if (!updatesAllowed) return
        pendingInstall?.let { f ->
            pendingInstall = null
            if (canInstall()) install(f)
        }
        if (settings.checkUpdates && lastCheck != 0L && SystemClock.elapsedRealtime() - lastCheck >= sixHours) {
            viewModelScope.launch { checkUpdate(manual = false) }
        }
    }

    fun setCheckUpdates(on: Boolean) = updateSettings { it.copy(checkUpdates = on) }

    /** Check now in the settings. It says "Quire is up to date." or shows the Update button. */
    fun checkNow() {
        if (!updatesAllowed || checking) return
        checking = true
        updateNote = null
        viewModelScope.launch {
            checkUpdate(manual = true)
            checking = false
        }
    }

    /** Asks GitHub for the latest release. A failed check stays quiet and tries again later. */
    private suspend fun checkUpdate(manual: Boolean) {
        lastCheck = SystemClock.elapsedRealtime()
        val got = withContext(Dispatchers.IO) {
            try {
                Result.success(fetchLatest())
            } catch (e: Exception) {
                Result.failure(e)
            }
        }
        got.onSuccess { r ->
            update = r?.takeIf { Update.newer(it.version, appVersion) }
            if (manual && update == null) updateNote = "Quire is up to date."
        }
    }

    /** The latest release with an APK, or null. Sends only the two headers that the API asks for. */
    private fun fetchLatest(): Release? {
        val c = java.net.URI(Update.LATEST).toURL().openConnection() as java.net.HttpURLConnection
        try {
            c.connectTimeout = 15_000
            c.readTimeout = 30_000
            c.useCaches = false
            c.setRequestProperty("Accept", "application/vnd.github+json")
            c.setRequestProperty("User-Agent", "quire-updater")
            if (c.responseCode != 200) throw IOException("The server answered ${c.responseCode}.")
            val text = c.inputStream.use { it.readBytes().toString(Charsets.UTF_8) }
            return Update.parse(text)
        } finally {
            c.disconnect()
        }
    }

    /** The Update button: downloads the APK to the cache folder, then opens the system installer. */
    fun startUpdate() {
        val r = update ?: return
        if (updating) return
        updating = true
        viewModelScope.launch {
            val f = withContext(Dispatchers.IO) {
                try {
                    download(r)
                } catch (e: Exception) {
                    null
                }
            }
            updating = false
            if (f == null) {
                flash("Quire could not download the update. Try again later.")
                return@launch
            }
            install(f)
        }
    }

    private fun download(r: Release): File {
        val dir = updatesDir()
        dir.mkdirs()
        val out = File(dir, "quire-${r.version}-android.apk")
        val part = File(dir, out.name + ".part")
        val c = java.net.URI(r.url).toURL().openConnection() as java.net.HttpURLConnection
        try {
            c.connectTimeout = 15_000
            c.readTimeout = 60_000
            // GitHub sends the file from another https host. HttpURLConnection follows that.
            c.instanceFollowRedirects = true
            c.setRequestProperty("User-Agent", "quire-updater")
            if (c.responseCode != 200) throw IOException("The server answered ${c.responseCode}.")
            c.inputStream.use { input ->
                part.outputStream().use { o ->
                    val buf = ByteArray(64 * 1024)
                    var total = 0L
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        total += n
                        if (total > 512L * 1024 * 1024) throw IOException("The file is too large.")
                        o.write(buf, 0, n)
                    }
                }
            }
        } finally {
            c.disconnect()
        }
        if (part.length() == 0L || !part.renameTo(out)) throw IOException("The download is not complete.")
        return out
    }

    private fun canInstall(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.O || ctx.packageManager.canRequestPackageInstalls()

    /**
     * Opens the system installer for [f] with a FileProvider URI. Without the permission to
     * install apps, it opens that setting first, and the install starts when the app comes back.
     */
    private fun install(f: File) {
        try {
            if (!canInstall()) {
                pendingInstall = f
                val i = Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${ctx.packageName}"))
                ctx.startActivity(i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                return
            }
            val uri = FileProvider.getUriForFile(ctx, "${ctx.packageName}.updates", f)
            val i = Intent(Intent.ACTION_VIEW)
                .setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            ctx.startActivity(i)
        } catch (e: Exception) {
            flash("Quire could not open the installer. Try again later.")
        }
    }

    // ---- Pinned notes ----------------------------------------------------------------------

    /** Pinned notes of the open folder, in the order they were pinned. */
    var pinned by mutableStateOf<List<String>>(emptyList()); private set

    /** The Pinned group of the tree is open. */
    var pinsOpen by mutableStateOf(true); private set

    fun isPinned(p: String): Boolean = p in pinned

    /** Pins a note to the top of the tree, or takes the pin away. A new pin opens the group. */
    fun togglePin(p: String) {
        if (p !in files) return
        val was = p in pinned
        pinned = Pins.toggle(pinned, p)
        if (!was) pinsOpen = true
        persistSession()
        // Pins sync, so a pin or an unpin syncs a moment later, unless sync is manual.
        syncAuto(1500)
    }

    fun togglePinsOpen() {
        pinsOpen = !pinsOpen
        persistSession()
    }

    // ---- Locked notes ----------------------------------------------------------------------

    /** The key and the plain text of a note that is unlocked now. */
    class Unlocked(val key: LockKey, val text: String)

    /** The unlocked notes. Only these hold plain text. The folder copy of each stays sealed. */
    var unlocked by mutableStateOf<Map<String, Unlocked>>(emptyMap()); private set

    /** True while the dialog for a new password shows. */
    var lockDialog by mutableStateOf(false)

    /** The error of the lock dialog, or null. */
    var lockError by mutableStateOf<String?>(null); private set

    /** The error under the password field of the lock panel, or null. */
    var unlockError by mutableStateOf<String?>(null); private set

    /** True while PBKDF2 runs for a lock or an unlock. */
    var lockBusy by mutableStateOf(false); private set

    /** True when the note is locked and not open now. */
    fun noteLocked(p: String): Boolean = p !in unlocked && files[p]?.let { Lock.isLocked(it) } == true

    /** The text of a note for the editor and the preview: plain while unlocked, and nothing while locked. */
    fun textOf(p: String): String = unlocked[p]?.text ?: files[p]?.takeUnless { Lock.isLocked(it) } ?: ""

    /** The text of the active note. */
    fun activeText(): String = active?.let { textOf(it) } ?: ""

    /** Opens the dialog that asks for a new password. On an unlocked note, it locks the note again at once. */
    fun askLock() {
        val a = active ?: return
        if (a in unlocked) {
            relock()
            return
        }
        if (noteLocked(a)) return
        lockError = null
        lockDialog = true
    }

    /** Locks the active note with [pass]. The note stays open, and each save seals it again. */
    fun lockNote(pass: String, repeat: String) {
        if (lockBusy) return
        if (pass.codePointCount(0, pass.length) < Lock.MIN_PASSWORD) {
            lockError = "The password needs at least ${Lock.MIN_PASSWORD} characters."
            return
        }
        if (pass != repeat) {
            lockError = "The two passwords are not the same."
            return
        }
        val a = active ?: return
        lockBusy = true
        viewModelScope.launch {
            // PBKDF2 with 600000 iterations takes a moment on a phone, so it runs off the main thread.
            val key = withContext(Dispatchers.Default) { Lock.newKey(pass) }
            lockBusy = false
            lockDialog = false
            lockError = null
            if (a !in files || noteLocked(a) || a in unlocked) return@launch
            val text = textOf(a)
            unlocked = unlocked + (a to Unlocked(key, text))
            files = files + (a to Lock.seal(key, text))
            dirty.add(a)
            flushAll()
            if (active == a && mode == Mode.Source) pendingEditorFocus = true
        }
    }

    /** Locks the active note again: its plain text leaves memory. */
    fun relock() {
        val a = active ?: return
        if (a !in unlocked) return
        flushAll()
        unlocked = unlocked - a
        unlockError = null
        loadEditor(a, null)
    }

    /** Opens the active locked note with [pass]. */
    fun unlock(pass: String) {
        if (lockBusy) return
        val a = active ?: return
        val text = files[a]?.takeIf { Lock.isLocked(it) } ?: return
        lockBusy = true
        viewModelScope.launch {
            val r = withContext(Dispatchers.Default) {
                try {
                    Result.success(Lock.open(text, pass))
                } catch (e: SyncException) {
                    Result.failure(e)
                }
            }
            lockBusy = false
            r.onSuccess { o ->
                unlockError = null
                // The note may have changed while the key was made. Then the password must open the new text.
                if (files[a] != text) return@onSuccess
                unlocked = unlocked + (a to Unlocked(o.key, o.text))
                if (active == a) {
                    loadEditor(a, null)
                    if (mode == Mode.Source) pendingEditorFocus = true
                }
            }.onFailure { e -> unlockError = e.message }
        }
    }

    /** Saves the active unlocked note as plain text again. */
    fun removePassword() {
        val a = active ?: return
        val u = unlocked[a] ?: return
        unlocked = unlocked - a
        files = files + (a to u.text)
        dirty.add(a)
        flushAll()
    }

    /**
     * An unlocked note changed outside the app, through sync or another app. The new text opens
     * with the key in memory, or the note locks again.
     */
    private fun outsideLockedText(p: String, text: String) {
        val u = unlocked[p] ?: return
        files = files + (p to text)
        if (!Lock.isLocked(text)) {
            // The password was removed elsewhere: the note is plain now.
            unlocked = unlocked - p
            if (active == p && editor.text != text) editor = TextFieldValue(text, shiftSelection(editor, text))
            return
        }
        try {
            val plain = Lock.openWith(u.key, text)
            unlocked = unlocked + (p to Unlocked(u.key, plain))
            if (active == p && editor.text != plain) editor = TextFieldValue(plain, shiftSelection(editor, plain))
        } catch (e: SyncException) {
            unlocked = unlocked - p
            if (active == p) loadEditor(p, null)
            flash("${Md.base(p)} has another password now. Unlock it again.")
        }
    }

    // ---- Images ----------------------------------------------------------------------------

    /** The image files in the open notes folder, with their stamps. */
    var images by mutableStateOf<Map<String, Stamp>>(emptyMap()); private set

    /** True when the photo picker must open. The screen opens it and sets this back. */
    var pickImage by mutableStateOf(false)

    /** Opens the photo picker for the active note. */
    fun requestImagePick() {
        val a = active
        if (a == null) {
            flash("Open a note first, then add the image to it.")
            return
        }
        if (noteLocked(a)) {
            flash("Unlock the note first, then add the image to it.")
            return
        }
        pickImage = true
    }

    /** Where the pixels of an image in the active note come from. */
    fun imageSource(img: Img): ImageSource = Images.source(img, active?.let { Md.dir(it) } ?: "", images.keys)

    /** A key for the image cache. It changes when the file changes. */
    fun imageKey(src: ImageSource): String = when (src) {
        is ImageSource.Vault -> "vault:${vault?.id}:${src.path}:${images[src.path]?.time}:${images[src.path]?.size}"
        is ImageSource.Path -> "path:${src.path}:${File(src.path).lastModified()}"
        is ImageSource.Url -> "url:${src.url}"
        is ImageSource.Missing -> "missing:${src.src}"
    }

    /** The bytes of an image. Call it off the main thread. Web images download, up to 50 MB. */
    fun imageBytes(src: ImageSource): ByteArray = when (src) {
        is ImageSource.Vault -> (fs ?: throw IOException("No folder is open.")).readBytes(src.path)
        is ImageSource.Path -> File(src.path).also { if (it.length() > Images.MAX_BYTES) throw IOException("The image is larger than 50 MB.") }.readBytes()
        is ImageSource.Url -> download(src.url)
        is ImageSource.Missing -> throw FileNotFoundException("No image at ${src.src}")
    }

    private fun download(url: String): ByteArray {
        val c = java.net.URI(url).toURL().openConnection() as java.net.HttpURLConnection
        try {
            c.connectTimeout = 10_000
            c.readTimeout = 30_000
            val code = c.responseCode
            if (code !in 200..299) throw IOException("The server answered $code.")
            val out = java.io.ByteArrayOutputStream()
            c.inputStream.use { input ->
                val buf = ByteArray(16 * 1024)
                while (true) {
                    val n = input.read(buf)
                    if (n < 0) break
                    out.write(buf, 0, n)
                    if (out.size() > Images.MAX_BYTES) throw IOException("The image is larger than 50 MB.")
                }
            }
            return out.toByteArray()
        } finally {
            c.disconnect()
        }
    }

    /**
     * Copies picked, pasted or dropped images into attachments/ in the notes folder, then links
     * them at the caret of the active note, each on a line of its own.
     */
    fun insertImages(uris: List<Uri>) {
        val f = fs ?: return
        val note = active
        if (note == null) {
            flash("Open a note first, then add the image to it.")
            return
        }
        if (noteLocked(note)) {
            flash("Unlock the note first, then add the image to it.")
            return
        }
        if (uris.isEmpty()) return
        val known = images.keys.toMutableSet()
        viewModelScope.launch {
            var message: String? = null
            val added = LinkedHashMap<String, Stamp>()
            withContext(io) {
                for (u in uris) {
                    var display: String? = null
                    var size = -1L
                    try {
                        ctx.contentResolver.query(u, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { c ->
                            if (c.moveToFirst()) {
                                display = c.getString(0)
                                if (!c.isNull(1)) size = c.getLong(1)
                            }
                        }
                    } catch (e: Exception) {
                        // Some sources have no name or size. The type and the bytes still tell.
                    }
                    val stamp = java.time.LocalDateTime.now().format(DateTimeFormatter.ofPattern("yyyy-MM-dd HH.mm.ss", Locale.ROOT))
                    val name = Images.nameFor(display, ctx.contentResolver.getType(u), stamp)
                    if (name == null) {
                        message = "Only images can go into a note: PNG, JPEG, GIF, WebP, BMP, SVG or AVIF."
                        continue
                    }
                    if (size > Images.MAX_BYTES) {
                        message = "$name is larger than 50 MB, so Quire did not add it."
                        continue
                    }
                    try {
                        val bytes = ctx.contentResolver.openInputStream(u)?.use { input ->
                            val out = java.io.ByteArrayOutputStream()
                            val buf = ByteArray(16 * 1024)
                            while (true) {
                                val n = input.read(buf)
                                if (n < 0) break
                                out.write(buf, 0, n)
                                if (out.size() > Images.MAX_BYTES) throw IOException("$name is larger than 50 MB, so Quire did not add it.")
                            }
                            out.toByteArray()
                        } ?: throw IOException("Quire can't read $name.")
                        val file = Md.uniqueName(name) { "${Images.ATTACHMENTS}/$it" in known }
                        val rel = "${Images.ATTACHMENTS}/$file"
                        f.writeBytes(rel, bytes)
                        known.add(rel)
                        added[rel] = Stamp(bytes.size.toLong(), System.currentTimeMillis())
                    } catch (e: Exception) {
                        message = e.message ?: "Quire could not add $name."
                    }
                }
            }
            message?.let { flash(it) }
            if (added.isEmpty()) return@launch
            images = images + added
            folders = folders + Images.ATTACHMENTS
            linkImages(added.keys.toList(), note)
            syncAfterSave()
        }
    }

    /** Writes `![name](path)` lines at the caret, each on a line of its own. */
    private fun linkImages(rels: List<String>, note: String) {
        if (active == note) {
            val v = editor
            val a = v.selection.min.coerceIn(0, v.text.length)
            val b = v.selection.max.coerceIn(0, v.text.length)
            val before = if (a > 0) v.text[a - 1] else null
            val after = if (b < v.text.length) v.text[b] else null
            val ins = Images.linkText(rels, Md.dir(note), before, after)
            applyEdit(TextFieldValue(v.text.substring(0, a) + ins + v.text.substring(b), TextRange(a + ins.length)))
            if (mode == Mode.Source) pendingEditorFocus = true
        } else {
            // The note is no longer open, so the images go at its end.
            val t = files[note] ?: return
            setDoc(note, t + Images.linkText(rels, Md.dir(note), t.lastOrNull(), null))
        }
    }

    /** A paste of copied images into the editor links them. False when the clipboard holds text or no image. */
    private fun pasteImages(): Boolean {
        val cm = ctx.getSystemService(ClipboardManager::class.java) ?: return false
        val clip = cm.primaryClip ?: return false
        val items = (0 until clip.itemCount).map { clip.getItemAt(it) }
        if (items.any { !it.text.isNullOrBlank() }) return false
        val uris = items.mapNotNull { it.uri }.filter { u ->
            (try {
                ctx.contentResolver.getType(u)
            } catch (e: Exception) {
                null
            })?.startsWith("image/") == true || Md.isImage(u.lastPathSegment ?: "")
        }
        if (uris.isEmpty()) return false
        insertImages(uris)
        return true
    }

    // ---- Sync ------------------------------------------------------------------------------

    /** The remote for the open folder, from the settings. Null when sync is off. */
    private fun remote(): Remote? {
        val sy = settings.sync
        if (sy.provider == "none") return null
        val cfg = (sy.cfg[sy.provider] ?: emptyMap()) - "prefix"
        return Remote(sy.provider, cfg, SyncEngine.cleanPrefix(remoteFolder()), sy.encrypt, sy.pass)
    }

    /** The remote folder of the open notes folder, as the settings show it. */
    fun remoteFolder(): String {
        val v = vault ?: return "quire/notes/"
        return settings.sync.folders[v.id]?.takeIf { it.isNotBlank() } ?: "quire/${v.name}/"
    }

    /** Sync is set up, its test passed and a folder is open. */
    fun syncReady(): Boolean {
        val sy = settings.sync
        return sy.provider != "none" && sy.connected && vault != null && fs != null
    }

    // The base record stays on this phone and out of backups.
    private fun baseFile(r: Remote, ref: VaultRef) = File(File(ctx.noBackupFilesDir, "sync"), r.id(ref.location) + ".json")

    private fun syncSoon(delayMs: Long) {
        if (!syncReady()) return
        val now = SystemClock.elapsedRealtime()
        val due = now + delayMs
        val cur = syncDue
        if (cur != null && cur <= due && syncTimer?.isActive == true) return
        syncDue = due
        syncTimer?.cancel()
        syncTimer = viewModelScope.launch {
            delay(due - now)
            syncTimer = null
            syncDue = null
            startSync()
        }
    }

    /** A sync that the app starts by itself. "Manually" skips it. */
    private fun syncAuto(delayMs: Long) {
        if (settings.sync.whenSync != "manual") syncSoon(delayMs)
    }

    /** After a save, "On save" syncs a moment after the typing stops. */
    private fun syncAfterSave() {
        if (settings.sync.whenSync == "save") syncSoon(1500)
    }

    fun syncNow() {
        if (!syncReady()) return
        syncManual = true
        syncStatus = SyncStatus("testing", "Syncing…")
        startSync()
    }

    private fun startTicker() {
        if (tickJob?.isActive == true) return
        tickJob = viewModelScope.launch {
            while (true) {
                syncClock = System.currentTimeMillis()
                if (settings.sync.whenSync == "interval" && syncReady() && syncJob == null && syncDue == null) {
                    val last = lastTry
                    syncSoon(if (last == null) 0 else max(0L, last + 300_000 - SystemClock.elapsedRealtime()))
                }
                delay(30_000)
            }
        }
    }

    private fun syncText(e: Throwable): String = when (e) {
        is SyncException -> e.message ?: "The sync failed."
        else -> ("The sync failed. " + (e.message ?: e.javaClass.simpleName)).trim()
    }

    private fun startSync() {
        syncTimer?.cancel()
        syncTimer = null
        syncDue = null
        lastTry = SystemClock.elapsedRealtime()
        if (syncJob != null) {
            syncAgain = true
            return
        }
        val r = remote()
        val f = fs
        val ref = vault
        if (r == null || f == null || ref == null || !settings.sync.connected) {
            if (syncManual) syncStatus = SyncStatus()
            syncManual = false
            return
        }
        // Saved notes go to disk first. The sync reads the folder on the same thread, after the writes.
        flushAll(quiet = true)
        val device = Build.MODEL?.trim()?.ifEmpty { null } ?: "Android"
        val bf = baseFile(r, ref)
        // The pins this sync starts with. The app applies only what the sync changed.
        val sent = pinned.toSet()
        syncBusy = true
        syncJob = viewModelScope.launch {
            val res = try {
                val local = try {
                    withContext(io) { f.load() }
                } catch (e: Exception) {
                    throw SyncException("Could not read the folder. ${e.message ?: ""}".trim())
                }
                // An image is read only when its stamp changed since the last sync.
                val pics = local.images.mapValues { (p, st) -> LocalImage(st.size, st.time) { f.readBytes(p) } }
                val out = withContext(Dispatchers.IO) { SyncEngine.run(r, local.files, pics, local.folders, device, SyncEngine.loadBase(bf), sent) }
                Result.success(out to local)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Result.failure(e)
            }
            syncJob = null
            syncBusy = false
            res.onSuccess { (out, local) ->
                applySync(out, f, ref, bf, local, sent)
                syncError = null
                val now = System.currentTimeMillis()
                syncClock = now
                setSync { it.copy(lastSync = now) }
                if (syncManual) {
                    val n = files.size
                    syncStatus = SyncStatus("ok", "Synced $n ${if (n == 1) "note" else "notes"} with ${out.place}.")
                }
            }.onFailure { e ->
                val msg = syncText(e)
                if (syncManual) syncStatus = SyncStatus("error", msg)
                syncError = msg
            }
            // After a sync that started by itself, the settings show the live state again.
            if (!syncManual && syncStatus.state == "ok") syncStatus = SyncStatus()
            syncManual = false
            if (syncAgain) {
                syncAgain = false
                syncSoon(500)
            }
        }
    }

    /**
     * Lays the outcome of a sync over the folder. A note that changed while the sync ran keeps its
     * new text, and the remote text comes in beside it as a conflict copy. The disk changes go
     * first, then the new base. If a write fails, the base goes, so the next sync loses no note.
     * A folder delete from a sync starts no other sync.
     */
    private fun applySync(out: Outcome, f: VaultFs, ref: VaultRef, bf: File, local: Loaded, sent: Set<String>) {
        val open = vault?.id == ref.id && fs === f
        applyPins(out.pins, sent, ref, open)
        val plan = if (open) {
            SyncEngine.plan(out, files, folders, dirty.toSet())
        } else {
            SyncEngine.plan(out, local.files, local.folders, emptySet())
        }
        ioScope.launch {
            var failed: String? = null
            for ((p, t) in plan.writes) {
                try {
                    f.write(p, t)
                } catch (e: Exception) {
                    failed = "Couldn't save ${Md.base(p)}. ${e.message ?: ""}".trim()
                }
            }
            for (p in plan.deletes) {
                try {
                    f.delete(p)
                } catch (e: Exception) {
                    failed = "Couldn't move ${Md.base(p)} to .trash. ${e.message ?: ""}".trim()
                }
            }
            // Images: written when the file here is still what the sync saw, or else as a copy.
            val known = if (open) withContext(Dispatchers.Main) { images.keys.toSet() } else local.images.keys
            val pics = SyncEngine.planImages(out, known) { p ->
                try {
                    Sign.sha256Hex(f.readBytes(p))
                } catch (e: Exception) {
                    null
                }
            }
            for ((p, b) in pics.writes) {
                try {
                    f.writeBytes(p, b)
                } catch (e: Exception) {
                    failed = "Couldn't save ${p.substringAfterLast('/')}. ${e.message ?: ""}".trim()
                }
            }
            for (p in pics.deletes) {
                try {
                    f.delete(p)
                } catch (e: Exception) {
                    failed = "Couldn't move ${p.substringAfterLast('/')} to .trash. ${e.message ?: ""}".trim()
                }
            }
            for (d in plan.mkdirs) {
                try {
                    f.mkdirs(d)
                } catch (e: Exception) {
                    failed = "Couldn't create the folder ${d.substringAfterLast('/')}. ${e.message ?: ""}".trim()
                }
            }
            for (d in plan.rmdirs) {
                try {
                    f.deleteDir(d)
                } catch (e: Exception) {
                    failed = "Couldn't move the folder ${d.substringAfterLast('/')} to .trash. ${e.message ?: ""}".trim()
                }
            }
            try {
                if (failed == null) SyncEngine.saveBase(bf, out.base) else bf.delete()
            } catch (e: Exception) {
                // The old base stays. The next sync compares with it, and no note is lost.
            }
            withContext(Dispatchers.Main) {
                if (open && vault?.id == ref.id && (pics.writes.isNotEmpty() || pics.deletes.isNotEmpty())) {
                    val now = System.currentTimeMillis()
                    val next = LinkedHashMap(images)
                    for ((p, b) in pics.writes) next[p] = Stamp(b.size.toLong(), now)
                    for (p in pics.deletes) next.remove(p)
                    images = next
                    folders = folders + pics.writes.flatMap { ancestors(it.first) }
                }
                val all = plan.conflicts + pics.conflicts
                if (all.isNotEmpty()) {
                    val who = if (all.size == 1) "A note" else "Some notes"
                    flash("$who changed on two devices. Quire kept both versions: ${all.joinToString(", ")}.")
                    // The copies go up on the next sync.
                    syncAuto(1000)
                }
                failed?.let { m -> flash(m) }
            }
        }
        if (open && (plan.writes.isNotEmpty() || plan.deletes.isNotEmpty() || plan.mkdirs.isNotEmpty() || plan.rmdirs.isNotEmpty())) {
            val nf = LinkedHashMap(files)
            var nd = folders
            val sealed = ArrayList<Pair<String, String>>()
            for ((p, t) in plan.writes) {
                // An unlocked note opens the new sealed text with its key, or locks again.
                if (p in unlocked) sealed.add(p to t) else nf[p] = t
                nd = nd + ancestors(p)
            }
            for (p in plan.deletes) {
                nf.remove(p)
                unlocked = unlocked - p
                pinned = Pins.remove(pinned, p)
            }
            for (d in plan.mkdirs) nd = nd + d + ancestors(d)
            setStructure(nf, nd)
            sealed.forEach { (p, t) -> outsideLockedText(p, t) }
            plan.rmdirs.forEach { dropFolder(it) }
            plan.rmdirs.forEach { pinned = Pins.removeIn(pinned, it) }
            val gone = plan.deletes.toSet()
            if (gone.isNotEmpty()) {
                val i = active?.let { tabs.indexOf(it) } ?: -1
                tabs = tabs.filter { it !in gone }
                recent = recent.filter { it !in gone }
                if (active in gone) {
                    active = if (tabs.isEmpty()) null else tabs[min(max(i, 0), tabs.size - 1)]
                    loadEditor(active, null)
                    if (active == null) focus = false
                }
            }
            val a = active
            val t = a?.let { if (it in unlocked) null else nf[it]?.takeUnless { x -> Lock.isLocked(x) } }
            if (t != null && a !in dirty && editor.text != t) editor = TextFieldValue(t, shiftSelection(editor, t))
            updateCount()
            persistSession()
        }
    }

    /**
     * Pins: what the sync added or removed, compared with the pins it sent. A pin that changed
     * here while the sync ran stays changed. New pins go at the end of the pin order. For a
     * folder that is no longer open, the change goes into its saved session.
     */
    private fun applyPins(after: Set<String>?, sent: Set<String>, ref: VaultRef, open: Boolean) {
        if (after == null) return
        fun delta(now: List<String>): List<String> {
            val next = now.filter { it in after || it !in sent }.toMutableList()
            for (p in after.sorted()) if (p !in sent && p !in next) next.add(p)
            return next
        }
        if (open) {
            val next = delta(pinned)
            if (next != pinned) {
                pinned = next
                persistSession()
            }
        } else {
            val ses = store.loadSession(ref.id)
            val next = delta(ses.pinned)
            if (next != ses.pinned) store.saveSession(ref.id, ses.copy(pinned = next))
        }
    }

    /** Checks the form, then reaches the store. It only reads, so it changes nothing remote. */
    fun testSync() {
        if (syncTesting) return
        val sy = settings.sync
        val cfg = syncCfg()
        val spec = Sync.spec(sy.provider, cfg, vault?.name ?: "")
        val missing = Sync.missing(spec, cfg)
        if (missing.isNotEmpty()) {
            syncStatus = SyncStatus("error", "Fill in " + missing.joinToString(", ") { it.label.lowercase() } + ".", missing.map { it.key })
            return
        }
        if (sy.encrypt && sy.pass.codePointCount(0, sy.pass.length) < 12) {
            syncStatus = SyncStatus("error", "The passphrase needs at least 12 characters.")
            return
        }
        val r = remote() ?: return
        val notes = files.size
        val before = sy.lastSync != null
        syncStatus = SyncStatus("testing", "Connecting…")
        syncTesting = true
        viewModelScope.launch {
            val res = try {
                Result.success(withContext(Dispatchers.IO) { SyncEngine.test(r, notes, before) })
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Result.failure(e)
            }
            syncTesting = false
            // The settings changed while the test ran, so its answer is for other settings.
            if (remote() != r) {
                syncStatus = SyncStatus()
                return@launch
            }
            res.onSuccess { msg ->
                setSync { it.copy(connected = true) }
                syncStatus = SyncStatus("ok", msg)
                syncAuto(200)
            }.onFailure { e ->
                setSync { it.copy(connected = false) }
                syncStatus = SyncStatus("error", syncText(e))
            }
        }
    }

    /** The sync state for the status bar: the label, busy and failed. Null when sync is off. */
    fun syncLabel(): Triple<String, Boolean, Boolean>? {
        val sy = settings.sync
        if (sy.provider == "none" || !sy.connected) return null
        if (syncBusy) return Triple("syncing…", true, false)
        if (syncError != null) return Triple("sync failed", false, true)
        val last = sy.lastSync ?: return Triple("not synced yet", false, false)
        return Triple("synced " + SyncEngine.ago(last, syncClock), false, false)
    }

    fun openSyncSettings() {
        setSec = "sync"
        openSettings()
    }

    // ---- Command palette and quick switcher -------------------------------------------------

    fun commands(): List<Cmd> {
        val has = active != null
        val locked = active?.let { noteLocked(it) } == true
        val open = active?.let { it in unlocked } == true
        val dark = isDark()
        return listOfNotNull(
            Cmd("New note") { newNote() },
            Cmd("New folder") { startNewFolder("") },
            Cmd("Open today's journal entry") { openToday() },
            Cmd("Open quick switcher") { openPalette("files") },
            if (has) Cmd(if (mode == Mode.Source) "Show preview" else "Show source") { toggleMode() } else null,
            Cmd("Search all notes") { openSearch() },
            if (has) Cmd(if (focus) "Leave focus mode" else "Enter focus mode") { toggleFocus() } else null,
            Cmd("Show sidebar") { drawerOpen = true },
            if (has) Cmd("Show outline") { outlineOpen = true } else null,
            Cmd(if (dark) "Use light theme" else "Use dark theme") { updateSettings { it.copy(theme = if (dark) "light" else "dark") } },
            if (has) Cmd("Rename note") { startRename() } else null,
            if (has) Cmd("Delete note") { deleteNote() } else null,
            if (has) Cmd("Close tab") { active?.let { closeTab(it) } } else null,
            if (has) Cmd("Insert image") { requestImagePick() } else null,
            if (has) Cmd(if (active?.let { isPinned(it) } == true) "Unpin note" else "Pin note") { active?.let { togglePin(it) } } else null,
            if (has && !locked && !open) Cmd("Lock note with a password…") { askLock() } else null,
            if (open) Cmd("Lock note now") { relock() } else null,
            if (open) Cmd("Remove password") { removePassword() } else null,
            if (syncReady()) Cmd("Sync now") { syncNow() } else null,
            Cmd("Open settings") { openSettings() },
            Cmd("Switch folder…") { goWelcome() },
            Cmd("Restore sample notes") { resetSample() },
        )
    }

    private fun segs(s: String, idx: List<Int>): List<Seg> {
        val set = idx.toHashSet()
        val out = ArrayList<Seg>()
        val cur = StringBuilder()
        var curHit: Boolean? = null
        for (i in s.indices) {
            val h = i in set
            if (curHit != null && curHit != h) {
                out.add(Seg(cur.toString(), curHit))
                cur.clear()
            }
            curHit = h
            cur.append(s[i])
        }
        if (curHit != null) out.add(Seg(cur.toString(), curHit))
        return out
    }

    fun palData(): PalData? {
        val p = palette ?: return null
        var m = p.mode
        var q = p.q
        if (m == "files" && q.startsWith(">")) {
            m = "commands"
            q = q.substring(1)
        }
        q = q.trim()
        val items = ArrayList<PalItem>()
        if (m == "files") {
            val paths = files.keys
            val list: List<Pair<String, List<Int>>> = if (q.isEmpty()) {
                val rec = recent.filter { it in files }
                (rec + paths.filter { it !in rec }.sorted()).map { it to emptyList() }
            } else {
                paths.mapNotNull { x ->
                    val f = Md.fuzzy(q, Md.base(x))
                    if (f != null) {
                        Triple(x, f.idx, f.score + 500)
                    } else {
                        Md.fuzzy(q, Md.stripExt(x))?.let { Triple(x, emptyList<Int>(), it.score) }
                    }
                }.sortedByDescending { it.third }.map { it.first to it.second }
            }
            list.take(50).forEach { (x, idx) -> items.add(PalItem(segs(Md.base(x), idx), Md.dir(x)) { openFile(x) }) }
            val typedPath = q.contains('/')
            val exists = if (typedPath) {
                val want = q.trim('/').removeSuffix(".md")
                paths.any { Md.stripExt(it).equals(want, ignoreCase = true) }
            } else {
                paths.any { Md.base(it).equals(q, ignoreCase = true) }
            }
            if (q.isNotEmpty() && !exists) {
                val name = q
                items.add(PalItem(listOf(Seg("Create note “$name”", false)), "new") {
                    palette = null
                    if (typedPath) newNoteAtPath(name) else newNote(name)
                })
            }
        } else {
            val cmds = commands()
            val list = if (q.isNotEmpty()) {
                cmds.mapNotNull { c -> Md.fuzzy(q, c.label)?.let { c to it.idx to it.score } }
                    .sortedByDescending { it.second }.map { it.first }
            } else {
                cmds.map { it to emptyList<Int>() }
            }
            list.forEach { (c, idx) ->
                items.add(PalItem(segs(c.label, idx), "") {
                    palette = null
                    c.run()
                })
            }
        }
        return PalData(m, items, p.idx.coerceIn(0, max(0, items.size - 1)))
    }

    fun setPaletteQuery(q: String) {
        palette = palette?.copy(q = q, idx = 0)
    }

    fun runPaletteItem() {
        val d = palData() ?: return
        d.items.getOrNull(d.idx)?.run?.invoke()
    }

    // ---- Back and hardware keys -------------------------------------------------------------

    /** Closes the top layer of the vault screen. Returns false when nothing was open. */
    fun onBack(): Boolean {
        when {
            vaultMenu -> vaultMenu = false
            newFolderParent != null -> newFolderParent = null
            drawerOpen -> drawerOpen = false
            outlineOpen -> outlineOpen = false
            renaming -> renaming = false
            focus -> focus = false
            else -> return false
        }
        return true
    }

    /** Handles the shortcuts from the prototype when a hardware keyboard is attached. */
    fun onKey(e: KeyEvent): Boolean {
        if (e.action != KeyEvent.ACTION_DOWN) return false
        val code = e.keyCode
        if (code == KeyEvent.KEYCODE_ESCAPE) {
            when {
                palette != null -> {
                    palette = null
                    if (active != null && mode == Mode.Source) pendingEditorFocus = true
                }
                settingsOpen -> settingsOpen = false
                vaultMenu -> vaultMenu = false
                newFolderParent != null -> newFolderParent = null
                drawerOpen -> drawerOpen = false
                outlineOpen -> outlineOpen = false
                renaming -> renaming = false
                creating != null -> creating = null
                focus -> focus = false
                else -> return false
            }
            return true
        }
        val p = palette
        if (p != null) {
            val d = palData() ?: return false
            val n = max(1, d.items.size)
            when (code) {
                KeyEvent.KEYCODE_DPAD_DOWN -> {
                    palette = p.copy(idx = (d.idx + 1) % n)
                    return true
                }
                KeyEvent.KEYCODE_DPAD_UP -> {
                    palette = p.copy(idx = (d.idx - 1 + n) % n)
                    return true
                }
                KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> {
                    d.items.getOrNull(d.idx)?.run?.invoke()
                    return true
                }
                KeyEvent.KEYCODE_DEL -> if (p.q.isEmpty() && p.mode == "commands") {
                    palette = p.copy(mode = "files", idx = 0)
                    return true
                }
            }
        }
        val mod = e.isCtrlPressed || e.isMetaPressed
        val inEditor = editorFocused && screen == Screen.Vault && mode == Mode.Source && palette == null && !settingsOpen
        if (!mod) {
            if (code == KeyEvent.KEYCODE_TAB && inEditor) {
                if (!e.isShiftPressed) insertText("  ")
                return true
            }
            return false
        }
        if (screen != Screen.Vault) return false
        if (e.isShiftPressed && code == KeyEvent.KEYCODE_F) {
            openSearch()
            return true
        }
        if (e.isShiftPressed || e.isAltPressed) return false
        when (code) {
            KeyEvent.KEYCODE_O -> openPalette("files")
            KeyEvent.KEYCODE_P -> openPalette("commands")
            KeyEvent.KEYCODE_E -> toggleMode(focusEditor = true)
            KeyEvent.KEYCODE_COMMA -> {
                settingsOpen = !settingsOpen
                palette = null
            }
            KeyEvent.KEYCODE_PERIOD -> toggleFocus()
            KeyEvent.KEYCODE_BACKSLASH -> drawerOpen = !drawerOpen
            KeyEvent.KEYCODE_N -> newNote()
            KeyEvent.KEYCODE_V -> if (!inEditor || !pasteImages()) return false
            KeyEvent.KEYCODE_B -> if (inEditor) wrapSelection("**") else return false
            KeyEvent.KEYCODE_I -> if (inEditor) wrapSelection("*") else return false
            else -> return false
        }
        return true
    }
}
