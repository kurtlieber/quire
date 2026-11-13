package app.quire.notes.core.sync

import com.sun.net.httpserver.HttpServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import java.net.InetSocketAddress
import java.util.TreeMap

/**
 * A small S3 server in the test JVM. It checks the signature of each request the way S3 does,
 * from the path and the headers that it got, then keeps the objects of one bucket in memory.
 */
class S3ServerTest {
    private lateinit var server: HttpServer
    private val objects = TreeMap<String, ByteArray>()
    private lateinit var url: String
    private val secret = "testsecret"

    /**
     * Answers 504 to the first try of every n-th request, as a gateway in front of a busy store
     * does. 0 never, 1 always. A try again of the same request gets through, so the test does
     * not depend on how the 8 parallel requests interleave.
     */
    @Volatile
    private var failEvery = 0
    private val count = java.util.concurrent.atomic.AtomicInteger(0)
    private val tries = java.util.concurrent.ConcurrentHashMap<String, Int>()

    @Before
    fun start() {
        // The tries again wait 1 ms to 8 ms in tests, not 0.5 s to 4 s.
        Http.waitScale = 0.002
        server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        server.executor = java.util.concurrent.Executors.newFixedThreadPool(16)
        server.createContext("/") { ex ->
            val method = ex.requestMethod
            val path = ex.requestURI.rawPath
            val body = ex.requestBody.readBytes()
            fun h(k: String) = ex.requestHeaders.getFirst(k) ?: ""
            val auth = h("Authorization")
            val date = h("x-amz-date")
            val region = auth.substringAfter("Credential=", "").split('/').getOrElse(2) { "" }
            val signed = listOf("host" to h("Host"), "x-amz-content-sha256" to h("x-amz-content-sha256"), "x-amz-date" to date)
            val want = Sign.SigV4("test", secret, region, "s3")
                .authorization(method, path, "", signed, h("x-amz-content-sha256"), date, date.take(8))
            val first = tries.merge("$method $path ${h("x-amz-content-sha256")}", 1, Int::plus) == 1
            val fail = failEvery == 1 || (failEvery > 1 && first && count.incrementAndGet() % failEvery == 0)
            val (code, out) = when {
                fail -> 504 to "<html><body><h1>504 Gateway Time-out</h1></body></html>".toByteArray()
                auth != want || h("x-amz-content-sha256") != Sign.sha256Hex(body) ->
                    403 to "<Error><Code>SignatureDoesNotMatch</Code></Error>".toByteArray()
                !path.startsWith("/notes/") -> 404 to "<Error><Code>NoSuchBucket</Code></Error>".toByteArray()
                else -> synchronized(objects) {
                    when (method) {
                        "GET" -> objects[path]?.let { 200 to it } ?: (404 to "<Error><Code>NoSuchKey</Code></Error>".toByteArray())
                        "PUT" -> {
                            objects[path] = body
                            200 to ByteArray(0)
                        }
                        "DELETE" -> {
                            objects.remove(path)
                            204 to ByteArray(0)
                        }
                        else -> 405 to ByteArray(0)
                    }
                }
            }
            ex.sendResponseHeaders(code, if (out.isEmpty()) -1 else out.size.toLong())
            if (out.isNotEmpty()) ex.responseBody.use { it.write(out) } else ex.close()
        }
        server.start()
        url = "http://127.0.0.1:${server.address.port}"
    }

    @After
    fun stop() = server.stop(0)

    private fun custom(endpoint: String, bucket: String, secret: String, encrypt: Boolean, prefix: String = "quire/Field Notes"): Remote {
        val cfg = mapOf(
            "endpoint" to endpoint, "bucket" to bucket, "accessKey" to "test", "secretKey" to secret,
            "addressing" to "path", "region" to "eu-north-1",
        )
        return Remote("custom", cfg, SyncEngine.cleanPrefix(prefix), encrypt, "a long enough passphrase")
    }

    @Test
    fun requestsAreSignedAndStored() {
        for (encrypt in listOf(false, true)) {
            val r = custom(url, "notes", secret, encrypt, if (encrypt) "quire/Secret Notes" else "quire/Field Notes")
            val a = Device("http-$encrypt", "laptop")
            val b = Device("http-$encrypt", "phone")
            a.write("Notater/Blåbær og fløte.md", "# Blåbær")
            a.write("Ideas (draft) #2.md", "# Ideas")
            val store = SyncEngine.openStore(r)
            assertEquals(2, a.sync(store, r).uploaded)
            val out = b.sync(store, r)
            assertEquals(2, out.pulls.size)
            assertEquals("# Blåbær", b.read("Notater/Blåbær og fløte.md"))
            val msg = SyncEngine.test(r, 2, true)
            assertTrue(msg, msg.startsWith("Connected to notes. It holds 2 notes"))
            val first = SyncEngine.test(r, 5, false)
            assertTrue(first, first.contains("holds 2 notes, and this folder has 5 notes. The first sync brings them together."))
            a.remove("Ideas (draft) #2.md")
            assertEquals(1, a.sync(store, r).deletedRemote)
            assertEquals(listOf("Ideas (draft) #2.md"), b.sync(store, r).removes.map { it.first })
        }
        val keys = objects.keys
        assertTrue(keys.toString(), "/notes/quire/Field%20Notes/Notater/Bl%C3%A5b%C3%A6r%20og%20fl%C3%B8te.md" in keys)
        assertTrue(keys.toString(), "/notes/quire/Field%20Notes/.quire/manifest.json" in keys)
        assertTrue(keys.toString(), "/notes/quire/Secret%20Notes/.quire/manifest.bin" in keys)
        assertTrue("the deleted note is gone: $keys", keys.none { it.contains("Ideas") })
        assertTrue("encrypted names hide paths: $keys", keys.none { it.startsWith("/notes/quire/Secret%20Notes/Notater") })
    }

    /** A gateway that drops one request in three still gives a full sync, and one that drops them all gives a message without its HTML page. */
    @Test
    fun aBusyGatewayIsRetried() {
        failEvery = 3
        val r = custom(url, "notes", secret, false)
        val a = Device("flaky", "laptop")
        val b = Device("flaky", "phone")
        for (i in 0 until 40) a.write("Note $i.md", "# $i")
        val store = SyncEngine.openStore(r)
        assertEquals(40, a.sync(store, r).uploaded)
        assertEquals(40, b.sync(store, r).pulls.size)
        assertTrue(objects.size >= 42)
        failEvery = 1
        try {
            SyncEngine.test(r, 0, false)
            fail("the test passed")
        } catch (e: SyncException) {
            assertEquals("The storage service did not answer in time (504). Quire tries again on the next sync.", e.message)
        }
    }

    @Test
    fun errorsSayWhatToFix() {
        fun err(r: Remote): String = try {
            SyncEngine.test(r, 0, false)
            fail("the test passed")
            ""
        } catch (e: SyncException) {
            e.message!!
        }
        assertEquals("Access denied (403). Check the secret access key.", err(custom(url, "notes", "wrong", false)))
        val bucket = err(custom(url, "other", secret, false))
        assertTrue(bucket, bucket.startsWith("The bucket does not exist"))
        assertEquals("Could not reach 127.0.0.1:9. The connection was refused or timed out.", err(custom("http://127.0.0.1:9", "notes", secret, false)))
        assertEquals("Could not reach the server. The endpoint is not a valid URL.", err(custom("", "notes", secret, false)))
        assertEquals("Could not reach nowhere.invalid. The host name does not resolve. Check the endpoint and the region.", err(custom("https://nowhere.invalid", "notes", secret, false)))
        assertEquals("Pick a storage backend first.", err(Remote("none", emptyMap(), "", false, "")))
        assertEquals(
            "The account key is not valid base64. Copy it again from the Azure portal.",
            err(Remote("azure", mapOf("account" to "a", "container" to "c", "azAuth" to "key", "azKey" to "not base64!"), "", false, "")),
        )
        assertEquals(
            "The service account key is not valid JSON. Paste the whole key file.",
            err(Remote("gcs", mapOf("bucket" to "b", "gcsJson" to "{nope"), "", false, "")),
        )
        assertEquals("The key file has no client_email.", err(Remote("gcs", mapOf("bucket" to "b", "gcsJson" to "{}"), "", false, "")))
    }
}
