#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <UIKit/UIKit.h>

static int g_once_val = 0;

int main(int argc, char **argv) {
    (void)argc;
    (void)argv;
    int mask = 0;

    // 1. CFConstantString literal + UTF8String
    NSString *s = @"pear-ios";
    if (strcmp([s UTF8String], "pear-ios") == 0) mask |= 1 << 0;

    // 2. NSMutableArray + NSNumber
    NSMutableArray *arr = [NSMutableArray array];
    [arr addObject:[NSNumber numberWithLongLong:77]];
    if ([arr count] == 1) mask |= 1 << 1;

    // 3. NSJSONSerialization round-trip
    NSData *jd = [@"{\"ok\":true}" dataUsingEncoding:NSUTF8StringEncoding];
    id parsed = [NSJSONSerialization JSONObjectWithData:jd options:0 error:nil];
    if (parsed != nil) mask |= 1 << 2;

    // 4. CoreGraphics CGRectInset
    CGRect r = CGRectInset((CGRect){{10, 20}, {100, 50}}, 5, 10);
    if (r.origin.x == 15 && r.size.width == 90) mask |= 1 << 3;

    // 5. UIKit UIApplication + UIDevice
    if ([UIApplication sharedApplication] != nil &&
        strcmp([[[UIDevice currentDevice] systemName] UTF8String], "iOS") == 0) {
        mask |= 1 << 4;
    }

    // 6. dispatch_once + arc4random
    static dispatch_once_t once;
    dispatch_once(&once, ^{ g_once_val = 99; });
    if (g_once_val == 99 && (arc4random() | 1u) != 0) mask |= 1 << 5;

    return mask;
}
