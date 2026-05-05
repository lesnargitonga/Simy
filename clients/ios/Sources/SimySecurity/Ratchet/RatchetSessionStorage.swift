import CryptoKit
import Foundation
import Security

public protocol RatchetSessionStorage {
    func loadSession(sessionId: String) async throws -> Data?
    func saveSession(sessionId: String, serializedSession: Data) async throws
    func deleteSession(sessionId: String) async throws
}

public final class IOSKeychainEncryptedRatchetStore: RatchetSessionStorage {
    private let rootDirectory: URL
    private let keyAlias: String

    public init(rootDirectory: URL, keyAlias: String) {
        self.rootDirectory = rootDirectory
        self.keyAlias = keyAlias
    }

    public func loadSession(sessionId: String) async throws -> Data? {
        try validateSessionId(sessionId)
        let path = sessionPath(for: sessionId)
        guard FileManager.default.fileExists(atPath: path.path) else {
            return nil
        }

        let encryptedRecord = try Data(contentsOf: path)
        return try decryptRecord(encryptedRecord)
    }

    public func saveSession(sessionId: String, serializedSession: Data) async throws {
        try validateSessionId(sessionId)
        try ensureRootDirectoryExists()

        let path = sessionPath(for: sessionId)
        let encryptedRecord = try encryptRecord(serializedSession)
        try encryptedRecord.write(to: path, options: [.atomic])
    }

    public func deleteSession(sessionId: String) async throws {
        try validateSessionId(sessionId)
        let path = sessionPath(for: sessionId)
        if FileManager.default.fileExists(atPath: path.path) {
            try FileManager.default.removeItem(at: path)
        }
    }

    private func ensureRootDirectoryExists() throws {
        if !FileManager.default.fileExists(atPath: rootDirectory.path) {
            try FileManager.default.createDirectory(at: rootDirectory, withIntermediateDirectories: true)
        }
    }

    private func sessionPath(for sessionId: String) -> URL {
        let hash = sha256Hex(sessionId)
        return rootDirectory.appendingPathComponent("\(hash).bin", isDirectory: false)
    }

    private func validateSessionId(_ sessionId: String) throws {
        if sessionId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            throw RatchetStorageError.invalidSessionId
        }
    }

    private func sha256Hex(_ value: String) -> String {
        let digest = SHA256.hash(data: Data(value.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private func encryptRecord(_ plaintext: Data) throws -> Data {
        let key = try loadOrCreateKey()
        let sealed = try AES.GCM.seal(plaintext, using: key)
        guard let combined = sealed.combined else {
            throw RatchetStorageError.encryptionFailure
        }

        var record = Data([recordVersion])
        record.append(combined)
        return record
    }

    private func decryptRecord(_ record: Data) throws -> Data {
        guard record.count > 1 else {
            throw RatchetStorageError.invalidRecord
        }

        let version = record.first!
        guard version == recordVersion else {
            throw RatchetStorageError.unsupportedRecordVersion
        }

        let combined = record.dropFirst()
        let sealed = try AES.GCM.SealedBox(combined: combined)
        return try AES.GCM.open(sealed, using: loadOrCreateKey())
    }

    private func loadOrCreateKey() throws -> SymmetricKey {
        if let existing = try readKeychainKey() {
            return SymmetricKey(data: existing)
        }

        var keyMaterial = Data(count: 32)
        let status = keyMaterial.withUnsafeMutableBytes { bytes in
            guard let baseAddress = bytes.baseAddress else {
                return errSecParam
            }
            return SecRandomCopyBytes(kSecRandomDefault, 32, baseAddress)
        }

        guard status == errSecSuccess else {
            throw RatchetStorageError.keyGenerationFailure
        }

        try writeKeychainKey(keyMaterial)
        return SymmetricKey(data: keyMaterial)
    }

    private func readKeychainKey() throws -> Data? {
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: keychainService,
            kSecAttrAccount: keyAlias,
            kSecReturnData: true,
            kSecMatchLimit: kSecMatchLimitOne,
        ]

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)

        if status == errSecItemNotFound {
            return nil
        }

        guard status == errSecSuccess, let data = item as? Data else {
            throw RatchetStorageError.keychainFailure(status)
        }

        return data
    }

    private func writeKeychainKey(_ keyData: Data) throws {
        let attributes: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: keychainService,
            kSecAttrAccount: keyAlias,
            kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
            kSecValueData: keyData,
        ]

        let status = SecItemAdd(attributes as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw RatchetStorageError.keychainFailure(status)
        }
    }

    private let keychainService = "com.simy.ratchet.storage"
    private let recordVersion: UInt8 = 1
}

public enum RatchetStorageError: Error {
    case invalidSessionId
    case invalidRecord
    case unsupportedRecordVersion
    case encryptionFailure
    case keyGenerationFailure
    case keychainFailure(OSStatus)
}
