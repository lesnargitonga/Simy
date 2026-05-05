package com.simy.security.ratchet

import java.io.File
import java.io.IOException
import java.nio.ByteBuffer
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties

/**
 * Android-side storage boundary that mirrors the Rust DoubleRatchetSessionStore trait.
 * The payload is an opaque serialized session blob produced/consumed by the Rust core.
 */
interface RatchetSessionStorage {
    suspend fun loadSession(sessionId: String): ByteArray?
    suspend fun saveSession(sessionId: String, serializedSession: ByteArray)
    suspend fun deleteSession(sessionId: String)
}

/**
 * Skeleton file-backed encrypted store for Android wrappers.
 *
 * Intended behavior:
 * - validate non-blank session IDs
 * - hash session IDs into stable filenames
 * - encrypt serialized session bytes with Android Keystore-backed keys
 * - atomically persist records
 */
class AndroidEncryptedRatchetStore(
    private val rootDir: File,
    private val keyAlias: String,
) : RatchetSessionStorage {

    private val secureRandom = SecureRandom()

    override suspend fun loadSession(sessionId: String): ByteArray? {
        validateSessionId(sessionId)
        val path = sessionPath(sessionId)
        if (!path.exists()) {
            return null
        }

        val encryptedBytes = path.readBytes()
        return decryptRecord(encryptedBytes)
    }

    override suspend fun saveSession(sessionId: String, serializedSession: ByteArray) {
        validateSessionId(sessionId)
        if (!rootDir.exists() && !rootDir.mkdirs()) {
            throw IOException("failed to create ratchet session directory: ${rootDir.absolutePath}")
        }

        val path = sessionPath(sessionId)
        val tempPath = File(path.parentFile, path.name + ".tmp")

        val encryptedBytes = encryptRecord(serializedSession)
        tempPath.writeBytes(encryptedBytes)

        if (path.exists() && !path.delete()) {
            tempPath.delete()
            throw IOException("failed to replace existing ratchet session file: ${path.absolutePath}")
        }

        if (!tempPath.renameTo(path)) {
            tempPath.delete()
            throw IOException("failed to atomically persist ratchet session file: ${path.absolutePath}")
        }
    }

    override suspend fun deleteSession(sessionId: String) {
        validateSessionId(sessionId)
        val path = sessionPath(sessionId)
        if (path.exists() && !path.delete()) {
            throw IOException("failed to delete ratchet session file: ${path.absolutePath}")
        }
    }

    private fun sessionPath(sessionId: String): File {
        val hash = sha256Hex(sessionId)
        return File(rootDir, "$hash.bin")
    }

    private fun validateSessionId(sessionId: String) {
        if (sessionId.isBlank()) {
            throw IllegalArgumentException("sessionId must not be blank")
        }
    }

    private fun sha256Hex(value: String): String {
        val digest = MessageDigest.getInstance("SHA-256").digest(value.toByteArray())
        return digest.joinToString(separator = "") { byte -> "%02x".format(byte) }
    }

    private fun encryptRecord(plaintext: ByteArray): ByteArray {
        val nonce = ByteArray(GCM_NONCE_LENGTH_BYTES)
        secureRandom.nextBytes(nonce)

        val cipher = Cipher.getInstance(AES_GCM_TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateSecretKey(), GCMParameterSpec(GCM_TAG_LENGTH_BITS, nonce))
        val ciphertext = cipher.doFinal(plaintext)

        val buffer = ByteBuffer.allocate(1 + 1 + nonce.size + ciphertext.size)
        buffer.put(RECORD_VERSION)
        buffer.put(nonce.size.toByte())
        buffer.put(nonce)
        buffer.put(ciphertext)
        return buffer.array()
    }

    private fun decryptRecord(ciphertext: ByteArray): ByteArray {
        if (ciphertext.size < 3) {
            throw IllegalArgumentException("invalid encrypted ratchet record")
        }

        val buffer = ByteBuffer.wrap(ciphertext)
        val version = buffer.get()
        if (version != RECORD_VERSION) {
            throw IllegalArgumentException("unsupported encrypted ratchet record version: $version")
        }

        val nonceLength = buffer.get().toInt() and 0xFF
        if (nonceLength <= 0 || nonceLength > 32 || buffer.remaining() <= nonceLength) {
            throw IllegalArgumentException("invalid encrypted ratchet record nonce length")
        }

        val nonce = ByteArray(nonceLength)
        buffer.get(nonce)
        val sealedBytes = ByteArray(buffer.remaining())
        buffer.get(sealedBytes)

        val cipher = Cipher.getInstance(AES_GCM_TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, getOrCreateSecretKey(), GCMParameterSpec(GCM_TAG_LENGTH_BITS, nonce))
        return cipher.doFinal(sealedBytes)
    }

    private fun getOrCreateSecretKey(): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        val existing = keyStore.getKey(keyAlias, null)
        if (existing is SecretKey) {
            return existing
        }

        val keyGenerator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        val spec = KeyGenParameterSpec.Builder(
            keyAlias,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setRandomizedEncryptionRequired(true)
            .build()

        keyGenerator.init(spec)
        return keyGenerator.generateKey()
    }

    companion object {
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val AES_GCM_TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_NONCE_LENGTH_BYTES = 12
        private const val GCM_TAG_LENGTH_BITS = 128
        private const val RECORD_VERSION: Byte = 1
    }
}
