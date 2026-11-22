#import <Foundation/Foundation.h>
#import <Security/Security.h>
#import <objc/runtime.h>

typedef NSURLCredential *(*ClientCredentialFactory)(
    Class, SEL, SecIdentityRef, NSArray *, NSURLCredentialPersistence);
static ClientCredentialFactory system_factory;

static NSURLCredential *checked_client_credential(
    Class cls, SEL selector, SecIdentityRef identity, NSArray *certificates,
    NSURLCredentialPersistence persistence) {
    // After SecPKCS12Import fails, this game passes the original NSData as
    // the identity and an uninitialized value in the certificate array.
    if (!identity || CFGetTypeID(identity) != SecIdentityGetTypeID()) {
        return nil;
    }
    return system_factory(cls, selector, identity, certificates, persistence);
}

__attribute__((visibility("hidden")))
void PearInstallClientCredentialCompatibility(void) {
    if (![NSBundle.mainBundle.bundleIdentifier isEqualToString:@"com.playstation.runsackboyrun"]) {
        return;
    }
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        Method method = class_getClassMethod(NSURLCredential.class,
            @selector(credentialWithIdentity:certificates:persistence:));
        if (method) {
            system_factory = (ClientCredentialFactory)method_getImplementation(method);
            method_setImplementation(method, (IMP)checked_client_credential);
        }
    });
}
