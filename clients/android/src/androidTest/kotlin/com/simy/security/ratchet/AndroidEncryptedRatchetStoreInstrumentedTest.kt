package com.simy.security.ratchet

import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNull
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.UUID

@RunWith(AndroidJUnit4::class)
class AndroidEncryptedRatchetStoreInstrumentedTest {

    @Test
    fun saveLoadDeleteRoundTrip() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val root = File(context.filesDir, "ratchet-tests-" + UUID.randomUUID().toString())
        val alias = "simy.ratchet.test." + UUID.randomUUID().toString()
        val store = AndroidEncryptedRatchetStore(rootDir = root, keyAlias = alias)

        val sessionId = "session-" + UUID.randomUUID().toString()
        val payload = "android-ratchet-session".toByteArray(Charsets.UTF_8)

        store.saveSession(sessionId, payload)
        val restored = store.loadSession(sessionId)
        assertArrayEquals(payload, restored)

        store.deleteSession(sessionId)
        val deleted = store.loadSession(sessionId)
        assertNull(deleted)

        root.deleteRecursively()
    }

    @Test
    fun blankSessionIdIsRejected() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val root = File(context.filesDir, "ratchet-tests-" + UUID.randomUUID().toString())
        val alias = "simy.ratchet.test." + UUID.randomUUID().toString()
        val store = AndroidEncryptedRatchetStore(rootDir = root, keyAlias = alias)

        try {
            store.saveSession("   ", byteArrayOf(1, 2, 3))
            fail("expected IllegalArgumentException")
        } catch (_: IllegalArgumentException) {
            // expected
        } finally {
            root.deleteRecursively()
        }
    }
}
