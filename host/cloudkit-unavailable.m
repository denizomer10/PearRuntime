#import <CloudKit/CloudKit.h>
#import <Security/Security.h>
#import <objc/runtime.h>

static NSError *cloud_unavailable(void) {
    return [NSError errorWithDomain:@"CKErrorDomain" code:CKErrorNotAuthenticated userInfo:nil];
}

@interface PearUnavailableCloudDatabase : NSObject
@end

@implementation PearUnavailableCloudDatabase
- (void)addOperation:(CKDatabaseOperation *)operation {
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
        NSError *error = cloud_unavailable();
        if ([operation isKindOfClass:NSClassFromString(@"CKQueryOperation")]) {
            void (^complete)(CKQueryCursor *, NSError *) = ((CKQueryOperation *)operation).queryCompletionBlock;
            if (complete) complete(nil, error);
        } else if ([operation isKindOfClass:NSClassFromString(@"CKModifyRecordsOperation")]) {
            void (^complete)(NSArray *, NSArray *, NSError *) = ((CKModifyRecordsOperation *)operation).modifyRecordsCompletionBlock;
            if (complete) complete(nil, nil, error);
        } else if ([operation isKindOfClass:NSClassFromString(@"CKModifyRecordZonesOperation")]) {
            void (^complete)(NSArray *, NSArray *, NSError *) = ((CKModifyRecordZonesOperation *)operation).modifyRecordZonesCompletionBlock;
            if (complete) complete(nil, nil, error);
        } else if ([operation isKindOfClass:NSClassFromString(@"CKFetchRecordZonesOperation")]) {
            void (^complete)(NSDictionary *, NSError *) = ((CKFetchRecordZonesOperation *)operation).fetchRecordZonesCompletionBlock;
            if (complete) complete(nil, error);
        } else if ([operation isKindOfClass:NSClassFromString(@"CKFetchRecordsOperation")]) {
            void (^complete)(NSDictionary *, NSError *) = ((CKFetchRecordsOperation *)operation).fetchRecordsCompletionBlock;
            if (complete) complete(nil, error);
        }
        void (^finished)(void) = operation.completionBlock;
        if (finished) finished();
    });
}

- (void)deleteRecordWithID:(CKRecordID *)recordID completionHandler:(void (^)(CKRecordID *, NSError *))complete {
    if (complete) {
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
            complete(nil, cloud_unavailable());
        });
    }
}
@end

@interface PearUnavailableCloudContainer : NSObject
@property (nonatomic, copy) NSString *containerIdentifier;
@property (nonatomic, strong) PearUnavailableCloudDatabase *database;
@end

@implementation PearUnavailableCloudContainer
- (id)privateCloudDatabase { return self.database; }
- (id)publicCloudDatabase { return self.database; }
- (id)sharedCloudDatabase { return self.database; }

- (void)fetchUserRecordIDWithCompletionHandler:(void (^)(CKRecordID *, NSError *))complete {
    if (complete) {
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
            complete(nil, cloud_unavailable());
        });
    }
}

- (void)accountStatusWithCompletionHandler:(void (^)(CKAccountStatus, NSError *))complete {
    if (complete) {
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
            complete(CKAccountStatusNoAccount, nil);
        });
    }
}
@end

static id unavailable_container(id self, SEL selector, NSString *identifier) {
    PearUnavailableCloudContainer *container = [PearUnavailableCloudContainer new];
    container.containerIdentifier = identifier;
    container.database = [PearUnavailableCloudDatabase new];
    return container;
}

static id unavailable_default_container(id self, SEL selector) {
    return unavailable_container(self, selector, nil);
}

__attribute__((visibility("hidden")))
void PearInstallUnavailableCloudKit(void) {
    NSString *bundle = NSBundle.mainBundle.bundleIdentifier;
    if (![bundle isEqualToString:@"com.chucklefish.stardewvalley"] &&
        ![bundle isEqualToString:@"com.rovio.abreloaded"] &&
        ![bundle isEqualToString:@"com.hipsterwhale.crossycastle"]) {
        return;
    }
    SecTaskRef task = SecTaskCreateFromSelf(NULL);
    if (!task) return;
    // The iOS signature is replaced during preparation. Keep native behavior
    // if the running app still has iCloud entitlements.
    CFTypeRef containers = SecTaskCopyValueForEntitlement(task, CFSTR("com.apple.developer.icloud-container-identifiers"), NULL);
    CFTypeRef services = SecTaskCopyValueForEntitlement(task, CFSTR("com.apple.developer.icloud-services"), NULL);
    CFRelease(task);
    BOOL hasCloudEntitlement = containers || services;
    if (containers) CFRelease(containers);
    if (services) CFRelease(services);
    if (hasCloudEntitlement) return;

    Class container = NSClassFromString(@"CKContainer");
    if (!container) return;
    Method defaultMethod = class_getClassMethod(container, @selector(defaultContainer));
    Method namedMethod = class_getClassMethod(container, @selector(containerWithIdentifier:));
    if (defaultMethod && namedMethod) {
        method_setImplementation(defaultMethod, (IMP)unavailable_default_container);
        method_setImplementation(namedMethod, (IMP)unavailable_container);
    }
}
