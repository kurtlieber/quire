package app.quire.notes.data

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.quire.notes.core.Sync
import org.json.JSONArray
import org.json.JSONObject
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

data class SyncSettings(
    val provider: String = "none",
    val cfg: Map<String, Map<String, String>> = emptyMap(),
    val whenSync: String = "save",
    val encrypt: Boolean = false,
    val pass: String = "",
    /** Test connection passed with these settings. A change to the store settings clears it. */
    val connected: Boolean = false,
    /** When the last sync ended, in milliseconds since 1970. */
    val lastSync: Long? = null,
    /**
     * The remote folder of each notes folder, by the id of the notes folder. A notes folder
     * without an entry uses quire/<folder name>/, so two notes folders never share a remote
     * folder by accident.
     */
    val folders: Map<String, String> = emptyMap(),
)

/** Settings and their defaults, from initState() in the prototype. */
data class Settings(
    /** The mode: "light", "dark" or "system". */
    val theme: String = "light",
    /** The theme id for light mode. Null means Paper. */
    val lightTheme: String? = null,
    /** The theme id for dark mode. Null means Ink. */
    val darkTheme: String? = null,
    /** "#rrggbb", or empty for the accent of the theme. */
    val accent: String = "",
    val font: String = "plex",
    val size: Int = 16,
    val width: String = "readable",
    val previewFont: String = "serif",
    val spellcheck: Boolean = false,
    val continueLists: Boolean = true,
    val dimFocus: Boolean = true,
    val typewriter: Boolean = true,
    val updateLinks: Boolean = true,
    val newFolder: String = "",
    val sync: SyncSettings = SyncSettings(),
    /** Look for a newer release of Quire. Release builds only. */
    val checkUpdates: Boolean = true,
)

enum class VaultKind { LOCAL, SAF }

/** A vault in the recent list. [location] is an absolute path or a tree URI. */
data class VaultRef(
    val id: String,
    val name: String,
    val kind: VaultKind,
    val location: String,
    val path: String,
    val count: Int,
)

data class Session(
    val tabs: List<String> = emptyList(),
    val active: String? = null,
    val recent: List<String> = emptyList(),
    val expanded: Set<String> = emptySet(),
    /** Pinned notes, in the order they were pinned. They stay on this phone and do not sync. */
    val pinned: List<String> = emptyList(),
    /** The Pinned group of the tree is open. */
    val pinsOpen: Boolean = true,
)

private fun JSONArray.strings(): List<String> = (0 until length()).map { getString(it) }
private fun List<String>.json(): JSONArray = JSONArray().also { a -> forEach { a.put(it) } }

/** Plain preferences. Credentials go to [SecretStore]. */
class Store(ctx: Context) {
    private val sp = ctx.getSharedPreferences("quire", Context.MODE_PRIVATE)

    var sampleCreated: Boolean
        get() = sp.getBoolean("sampleCreated", false)
        set(v) = sp.edit().putBoolean("sampleCreated", v).apply()

    var lastVault: String?
        get() = sp.getString("lastVault", null)
        set(v) = sp.edit().putString("lastVault", v).apply()

    var lastScreen: String
        get() = sp.getString("lastScreen", "vault") ?: "vault"
        set(v) = sp.edit().putString("lastScreen", v).apply()

    var mode: String
        get() = sp.getString("mode", "source") ?: "source"
        set(v) = sp.edit().putString("mode", v).apply()

    fun loadSettings(): Settings {
        val o = try {
            JSONObject(sp.getString("settings", null) ?: return Settings())
        } catch (e: Exception) {
            return Settings()
        }
        val d = Settings()
        val sy = o.optJSONObject("sync")
        val cfg = HashMap<String, Map<String, String>>()
        sy?.optJSONObject("cfg")?.let { c ->
            for (pid in c.keys()) {
                val m = HashMap<String, String>()
                val po = c.getJSONObject(pid)
                // The remote folder used to be one value for every notes folder. Each folder has its own now.
                for (k in po.keys()) if (k != "prefix") m[k] = po.getString(k)
                cfg[pid] = m
            }
        }
        return Settings(
            theme = o.optString("theme", d.theme),
            lightTheme = o.optString("lightTheme", "").ifEmpty { null },
            darkTheme = o.optString("darkTheme", "").ifEmpty { null },
            accent = o.optString("accent", d.accent),
            font = o.optString("font", d.font),
            size = o.optInt("size", d.size).coerceIn(12, 24),
            width = o.optString("width", d.width),
            previewFont = o.optString("previewFont", d.previewFont),
            spellcheck = o.optBoolean("spellcheck", d.spellcheck),
            continueLists = o.optBoolean("continueLists", d.continueLists),
            dimFocus = o.optBoolean("dimFocus", d.dimFocus),
            typewriter = o.optBoolean("typewriter", d.typewriter),
            updateLinks = o.optBoolean("updateLinks", d.updateLinks),
            newFolder = o.optString("newFolder", d.newFolder),
            checkUpdates = o.optBoolean("checkUpdates", d.checkUpdates),
            sync = SyncSettings(
                provider = sy?.optString("provider", "none") ?: "none",
                cfg = cfg,
                whenSync = sy?.optString("when", "save") ?: "save",
                encrypt = sy?.optBoolean("encrypt", false) ?: false,
                connected = sy?.optBoolean("connected", false) ?: false,
                lastSync = sy?.optLong("lastSync", 0L)?.takeIf { it > 0 },
                folders = sy?.optJSONObject("folders")?.let { f -> f.keys().asSequence().associateWith { f.getString(it) } } ?: emptyMap(),
            ),
        )
    }

    /** Saves everything except credentials and the passphrase. */
    fun saveSettings(s: Settings) {
        val cfg = JSONObject()
        for ((pid, m) in s.sync.cfg) {
            val po = JSONObject()
            for ((k, v) in m) if (k !in Sync.secretKeys && k != "prefix") po.put(k, v)
            cfg.put(pid, po)
        }
        val o = JSONObject()
            .put("theme", s.theme).put("lightTheme", s.lightTheme ?: "").put("darkTheme", s.darkTheme ?: "").put("accent", s.accent)
            .put("font", s.font).put("size", s.size).put("width", s.width)
            .put("previewFont", s.previewFont).put("spellcheck", s.spellcheck).put("continueLists", s.continueLists)
            .put("dimFocus", s.dimFocus).put("typewriter", s.typewriter).put("updateLinks", s.updateLinks)
            .put("newFolder", s.newFolder)
            .put("checkUpdates", s.checkUpdates)
            .put(
                "sync", JSONObject().put("provider", s.sync.provider).put("when", s.sync.whenSync)
                    .put("encrypt", s.sync.encrypt).put("cfg", cfg)
                    .put("connected", s.sync.connected).put("lastSync", s.sync.lastSync ?: 0L)
                    .put("folders", JSONObject().also { f -> for ((k, v) in s.sync.folders) f.put(k, v) })
            )
        sp.edit().putString("settings", o.toString()).apply()
    }

    fun loadVaults(): List<VaultRef> = try {
        val a = JSONArray(sp.getString("vaults", "[]"))
        (0 until a.length()).map {
            val o = a.getJSONObject(it)
            VaultRef(
                id = o.getString("id"),
                name = o.getString("name"),
                kind = if (o.optString("kind") == "saf") VaultKind.SAF else VaultKind.LOCAL,
                location = o.getString("location"),
                path = o.optString("path"),
                count = o.optInt("count"),
            )
        }
    } catch (e: Exception) {
        emptyList()
    }

    fun saveVaults(list: List<VaultRef>) {
        val a = JSONArray()
        list.forEach {
            a.put(
                JSONObject().put("id", it.id).put("name", it.name)
                    .put("kind", if (it.kind == VaultKind.SAF) "saf" else "local")
                    .put("location", it.location).put("path", it.path).put("count", it.count)
            )
        }
        sp.edit().putString("vaults", a.toString()).apply()
    }

    fun loadSession(id: String): Session = try {
        val o = JSONObject(sp.getString("session.$id", null) ?: return Session())
        Session(
            tabs = o.optJSONArray("tabs")?.strings() ?: emptyList(),
            active = if (o.isNull("active")) null else o.optString("active"),
            recent = o.optJSONArray("recent")?.strings() ?: emptyList(),
            expanded = o.optJSONArray("expanded")?.strings()?.toSet() ?: emptySet(),
            pinned = o.optJSONArray("pinned")?.strings() ?: emptyList(),
            pinsOpen = !o.optBoolean("pinsClosed", false),
        )
    } catch (e: Exception) {
        Session()
    }

    fun saveSession(id: String, s: Session) {
        val o = JSONObject()
            .put("tabs", s.tabs.json())
            .put("active", s.active ?: JSONObject.NULL)
            .put("recent", s.recent.json())
            .put("expanded", s.expanded.toList().json())
            .put("pinned", s.pinned.json())
            .put("pinsClosed", !s.pinsOpen)
        sp.edit().putString("session.$id", o.toString()).apply()
    }

    fun forgetSession(id: String) {
        sp.edit().remove("session.$id").apply()
    }
}

/**
 * Keeps credentials encrypted with an AES key that lives in the Android Keystore.
 * The encrypted blob sits in its own preferences file, which backups leave out.
 */
class SecretStore(ctx: Context) {
    private val sp = ctx.getSharedPreferences("quire_secure", Context.MODE_PRIVATE)
    private val alias = "quire_secrets_v1"

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getEntry(alias, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val kg = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        kg.init(
            KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return kg.generateKey()
    }

    fun load(): Map<String, String> {
        val blob = sp.getString("blob", null) ?: return emptyMap()
        return try {
            val raw = Base64.decode(blob, Base64.NO_WRAP)
            val c = Cipher.getInstance("AES/GCM/NoPadding")
            c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, raw.copyOfRange(0, 12)))
            val o = JSONObject(String(c.doFinal(raw.copyOfRange(12, raw.size)), Charsets.UTF_8))
            o.keys().asSequence().associateWith { o.getString(it) }
        } catch (e: Exception) {
            emptyMap()
        }
    }

    fun save(values: Map<String, String>) {
        try {
            val o = JSONObject()
            values.forEach { (k, v) -> if (v.isNotEmpty()) o.put(k, v) }
            val c = Cipher.getInstance("AES/GCM/NoPadding")
            c.init(Cipher.ENCRYPT_MODE, key())
            val out = c.iv + c.doFinal(o.toString().toByteArray(Charsets.UTF_8))
            sp.edit().putString("blob", Base64.encodeToString(out, Base64.NO_WRAP)).apply()
        } catch (e: Exception) {
            // The Keystore can fail on a broken device. The secrets then stay in memory for this session only.
        }
    }
}
