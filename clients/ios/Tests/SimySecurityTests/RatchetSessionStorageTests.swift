import Foundation
import Security
import XCTest
@testable import SimySecurity

final class RatchetSessionStorageTests: XCTestCase {
    func testSaveLoadDeleteRoundTrip() async throws {
        let alias = "simy.ratchet.test.\(UUID().uuidString)"
        let root = temporaryRootDirectory()
        let store = IOSKeychainEncryptedRatchetStore(rootDirectory: root, keyAlias: alias)

        defer {
            try? FileManager.default.removeItem(at: root)
            removeKeychainKey(alias: alias)
        }

        let sessionId = "session-\(UUID().uuidString)"
        let payload = Data("ios-ratchet-session".utf8)

        try await store.saveSession(sessionId: sessionId, serializedSession: payload)
        let restored = try await store.loadSession(sessionId: sessionId)
        XCTAssertEqual(restored, payload)

        try await store.deleteSession(sessionId: sessionId)
        let deleted = try await store.loadSession(sessionId: sessionId)
        XCTAssertNil(deleted)
    }

    func testBlankSessionIdRejected() async throws {
        let alias = "simy.ratchet.test.\(UUID().uuidString)"
        let root = temporaryRootDirectory()
        let store = IOSKeychainEncryptedRatchetStore(rootDirectory: root, keyAlias: alias)

        defer {
            try? FileManager.default.removeItem(at: root)
            removeKeychainKey(alias: alias)
        }

        do {
            try await store.saveSession(sessionId: " ", serializedSession: Data([1, 2, 3]))
            XCTFail("expected invalidSessionId")
        } catch RatchetStorageError.invalidSessionId {
            // expected
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    private func temporaryRootDirectory() -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    private func removeKeychainKey(alias: String) {
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: "com.simy.ratchet.storage",
            kSecAttrAccount: alias,
        ]
        SecItemDelete(query as CFDictionary)
    }
}
