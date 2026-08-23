import Foundation
import Security

enum AssistantAPIKeyStore {
    private static let service = "com.olledyberg.Koett.assistant"
    private static let cartesiaAccount = "cartesia"

    static func load(for provider: AssistantProvider) throws -> String? {
        try load(account: provider.rawValue)
    }

    static func loadCartesia() throws -> String? {
        try load(account: cartesiaAccount)
    }

    private static func load(account: String) throws -> String? {
        var result: CFTypeRef?
        let status = SecItemCopyMatching([
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecReturnData as String: true,
        ] as CFDictionary, &result)

        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess,
              let data = result as? Data,
              let key = String(data: data, encoding: .utf8) else {
            throw failure(status)
        }
        return key
    }

    static func save(_ key: String, for provider: AssistantProvider) throws {
        try save(key, account: provider.rawValue)
    }

    static func saveCartesia(_ key: String) throws {
        try save(key, account: cartesiaAccount)
    }

    private static func save(_ key: String, account: String) throws {
        let cleanKey = key.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !cleanKey.isEmpty else {
            throw NSError(
                domain: "Koett",
                code: 1,
                userInfo: [NSLocalizedDescriptionKey: "The API key is empty."]
            )
        }

        let match: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        let data = Data(cleanKey.utf8)
        var status = SecItemUpdate(
            match as CFDictionary,
            [kSecValueData as String: data] as CFDictionary
        )

        if status == errSecItemNotFound {
            var item = match
            item[kSecValueData as String] = data
            item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
            status = SecItemAdd(item as CFDictionary, nil)
        }

        guard status == errSecSuccess else {
            throw failure(status)
        }
    }

    private static func failure(_ status: OSStatus) -> NSError {
        let detail = SecCopyErrorMessageString(status, nil) as String?
            ?? "Keychain error \(status)"
        return NSError(
            domain: "Koett",
            code: Int(status),
            userInfo: [NSLocalizedDescriptionKey: detail]
        )
    }
}
