package app.quire.notes.core

import app.quire.notes.core.sync.Json

/** A release that has an Android APK: its version, for example 0.5.4, and the URL of the APK. */
data class Release(val version: String, val url: String)

/**
 * The update check: the latest release on GitHub, and a compare with the version of this app.
 * The app shows the Update button only for a newer release with an APK.
 */
object Update {
    /** The latest release. The API leaves out drafts and prereleases. */
    const val LATEST = "https://api.github.com/repos/bjarneo/quire/releases/latest"

    /** An APK URL must start with this, so the app downloads only from the releases of Quire. */
    const val DOWNLOADS = "https://github.com/bjarneo/quire/releases/download/"

    /** The version of a tag such as "v0.5.4" or "0.5.4". */
    fun version(tag: String): String = tag.trim().removePrefix("v").removePrefix("V")

    /** The 3 numbers of a version. A missing or bad part counts as 0, and text after the digits of a part is ignored. */
    private fun parts(v: String): List<Int> {
        val p = version(v).split('.')
        return (0 until 3).map { i -> p.getOrNull(i)?.takeWhile { it.isDigit() }?.toIntOrNull() ?: 0 }
    }

    /** Below 0 when [a] is older than [b], 0 when they are the same, and above 0 when [a] is newer. */
    fun compare(a: String, b: String): Int {
        val x = parts(a)
        val y = parts(b)
        for (i in 0 until 3) if (x[i] != y[i]) return x[i].compareTo(y[i])
        return 0
    }

    fun newer(latest: String, current: String): Boolean = compare(latest, current) > 0

    /**
     * The release in the JSON of the GitHub API, or null. It is null for a draft, a prerelease,
     * a release without the asset quire-<version>-android.apk, and an asset URL outside the
     * releases of Quire.
     */
    fun parse(json: String): Release? {
        val o = try {
            Json.parse(json) as? Map<*, *>
        } catch (e: Exception) {
            null
        } ?: return null
        if (o["draft"] == true || o["prerelease"] == true) return null
        val tag = o["tag_name"] as? String ?: return null
        val v = version(tag)
        if (v.isEmpty() || !v[0].isDigit()) return null
        val want = "quire-$v-android.apk"
        val assets = o["assets"] as? List<*> ?: return null
        for (a in assets) {
            val m = a as? Map<*, *> ?: continue
            if (m["name"] != want) continue
            val url = m["browser_download_url"] as? String ?: return null
            return if (url.startsWith(DOWNLOADS)) Release(v, url) else null
        }
        return null
    }
}
