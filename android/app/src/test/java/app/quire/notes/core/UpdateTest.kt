package app.quire.notes.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The update check: the version compare and the parse of the release JSON of GitHub. */
class UpdateTest {
    private fun release(tag: String, name: String, url: String, extra: String = "") = """
        {"tag_name": "$tag", "name": "Quire", "draft": false, "prerelease": false$extra,
         "assets": [
           {"name": "quire-0.5.4-linux-x86_64.tar.gz", "browser_download_url": "https://github.com/bjarneo/quire/releases/download/v0.5.4/quire-0.5.4-linux-x86_64.tar.gz"},
           {"name": "$name", "browser_download_url": "$url", "size": 10143458}
         ]}
    """.trimIndent()

    private val good = "https://github.com/bjarneo/quire/releases/download/v0.5.4/quire-0.5.4-android.apk"

    @Test
    fun versionsCompareAsThreeNumbers() {
        assertTrue(Update.newer("0.5.4", "0.5.3"))
        assertTrue(Update.newer("v0.10.0", "0.9.9"))
        assertTrue(Update.newer("1.0", "0.99.99"))
        assertFalse(Update.newer("0.5.3", "0.5.3"))
        assertFalse(Update.newer("v0.5.3", "0.5.3"))
        assertFalse(Update.newer("0.5.2", "0.5.3"))
        assertEquals(0, Update.compare("0.5", "0.5.0"))
        assertTrue(Update.compare("0.5.3-beta", "0.5.2") > 0)
        assertEquals("0.5.4", Update.version("v0.5.4"))
        assertEquals("0.5.4", Update.version("0.5.4"))
    }

    @Test
    fun aReleaseWithAnApkParses() {
        assertEquals(Release("0.5.4", good), Update.parse(release("v0.5.4", "quire-0.5.4-android.apk", good)))
        // A tag without "v" works too.
        assertEquals(Release("0.5.4", good), Update.parse(release("0.5.4", "quire-0.5.4-android.apk", good)))
    }

    @Test
    fun otherReleasesGiveNothing() {
        // No APK in the release.
        assertNull(Update.parse(release("v0.5.4", "quire-0.5.4-macos-arm64.zip", "https://github.com/bjarneo/quire/releases/download/v0.5.4/quire-0.5.4-macos-arm64.zip")))
        // An APK of another version.
        assertNull(Update.parse(release("v0.5.4", "quire-0.5.3-android.apk", good)))
        // A URL on another host, or outside the releases of Quire.
        assertNull(Update.parse(release("v0.5.4", "quire-0.5.4-android.apk", "https://example.com/bjarneo/quire/releases/download/v0.5.4/quire-0.5.4-android.apk")))
        assertNull(Update.parse(release("v0.5.4", "quire-0.5.4-android.apk", "https://github.com/someone/quire/releases/download/v0.5.4/quire-0.5.4-android.apk")))
        assertNull(Update.parse(release("v0.5.4", "quire-0.5.4-android.apk", "http://github.com/bjarneo/quire/releases/download/v0.5.4/quire-0.5.4-android.apk")))
        // A draft or a prerelease.
        assertNull(Update.parse(release("v0.5.4", "quire-0.5.4-android.apk", good).replace("\"prerelease\": false", "\"prerelease\": true")))
        assertNull(Update.parse(release("v0.5.4", "quire-0.5.4-android.apk", good).replace("\"draft\": false", "\"draft\": true")))
        // JSON that is not a release.
        assertNull(Update.parse("{\"message\": \"API rate limit exceeded\"}"))
        assertNull(Update.parse("not json"))
        assertNull(Update.parse("{\"tag_name\": \"nightly\", \"assets\": []}"))
    }
}
