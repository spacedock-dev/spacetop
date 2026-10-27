/* One exact legacy Keychain item reference. Never request data or attributes. */
#include <CoreFoundation/CoreFoundation.h>
#include <Security/Security.h>
#include <stdio.h>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    /* Process-local guard covers legacy Keychain prompts. Fail closed. */
    OSStatus set_status = SecKeychainSetUserInteractionAllowed(false);
    Boolean interaction = true;
    OSStatus get_status = SecKeychainGetUserInteractionAllowed(&interaction);
    if (set_status != errSecSuccess || get_status != errSecSuccess || interaction) {
        printf("{\"guard_set_status\":%d,\"guard_get_status\":%d,"
               "\"interaction_allowed\":%s,\"query_attempted\":false}\n",
               (int)set_status, (int)get_status, interaction ? "true" : "false");
        return 3;
    }
    CFStringRef service = CFStringCreateWithCString(NULL, argv[1], kCFStringEncodingUTF8);
    CFStringRef account = CFStringCreateWithCString(NULL, argv[2], kCFStringEncodingUTF8);
    if (!service || !account) return 4;
    const void *keys[] = {
        kSecClass, kSecAttrService, kSecAttrAccount, kSecMatchLimit,
        kSecReturnData, kSecReturnAttributes, kSecReturnRef,
        kSecReturnPersistentRef, kSecUseAuthenticationUI
    };
    const void *values[] = {
        kSecClassGenericPassword, service, account, kSecMatchLimitOne,
        kCFBooleanFalse, kCFBooleanFalse, kCFBooleanTrue,
        kCFBooleanFalse, kSecUseAuthenticationUIFail
    };
    CFDictionaryRef query = CFDictionaryCreate(NULL, keys, values, 9,
        &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    if (!query) return 5;
    CFTypeRef item = NULL;
    OSStatus status = SecItemCopyMatching(query, &item);
    printf("{\"guard_set_status\":0,\"guard_get_status\":0,"
           "\"interaction_allowed\":false,\"query_attempted\":true,"
           "\"os_status\":%d,\"opaque_reference_present\":%s}\n",
           (int)status, item ? "true" : "false");
    if (item) CFRelease(item);
    CFRelease(query);
    CFRelease(account);
    CFRelease(service);
    return 0;
}
