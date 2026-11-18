// Objective-C class symbols for UIKit classes implemented in Rust (pear-uikit).
// Methods are registered onto these classes at load time by pear_uikit_register_classes().

#import <Foundation/Foundation.h>
#import "../include/PearUIKit.h"

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wincomplete-implementation"

@implementation UIColor @end
@implementation UITraitCollection @end
@implementation UIResponder @end
@implementation UIScreen @end
@implementation UIDevice @end
@implementation UIView @end
@implementation UIViewController @end
@implementation UIWindow @end
@implementation UIScrollView @end
@implementation UITextView @end
@implementation UIAlertAction @end
@implementation UIAlertController @end
@implementation UINib @end
@implementation UIApplication @end
@implementation UITouch @end
@implementation UIEvent @end
@implementation UIKey @end
@implementation UIPress @end
@implementation UIPressesEvent @end
@implementation UINavigationController @end

@interface UIImage : NSObject @end
@implementation UIImage @end

@interface UIFont : NSObject @end
@implementation UIFont @end
Class pear_uikit_uifont_class(void) { return [UIFont class]; }

@interface UILabel : UIView @end
@implementation UILabel @end

@interface UIProgressView : UIView @end
@implementation UIProgressView @end

@interface UIPasteboard : NSObject @end
@implementation UIPasteboard @end

@interface UIFeedbackGenerator : NSObject @end
@implementation UIFeedbackGenerator @end

@interface UIImpactFeedbackGenerator : UIFeedbackGenerator @end
@implementation UIImpactFeedbackGenerator @end

@interface UINotificationFeedbackGenerator : UIFeedbackGenerator @end
@implementation UINotificationFeedbackGenerator @end

@interface UISelectionFeedbackGenerator : UIFeedbackGenerator @end
@implementation UISelectionFeedbackGenerator @end

@interface UIStoryboard : NSObject @end
@implementation UIStoryboard @end

@interface UIAccessibilityElement : UIResponder @end
@implementation UIAccessibilityElement @end

@interface UIPointerInteraction : NSObject @end
@implementation UIPointerInteraction @end

@interface UITextPosition : NSObject @end
@implementation UITextPosition @end

@interface UITextRange : NSObject @end
@implementation UITextRange @end

@interface UIUserNotificationAction : NSObject @end
@implementation UIUserNotificationAction @end

@interface UIMutableUserNotificationAction : UIUserNotificationAction @end
@implementation UIMutableUserNotificationAction @end

@interface UIUserNotificationCategory : NSObject @end
@implementation UIUserNotificationCategory @end

@interface UIMutableUserNotificationCategory : UIUserNotificationCategory @end
@implementation UIMutableUserNotificationCategory @end

@interface UIUserNotificationSettings : NSObject @end
@implementation UIUserNotificationSettings @end

@interface UIBarItem : NSObject @end
@implementation UIBarItem @end

@interface UIBarButtonItem : UIBarItem @end
@implementation UIBarButtonItem @end

@interface UITabBarController : UIViewController @end
@implementation UITabBarController @end

@interface UIActivityViewController : UIViewController @end
@implementation UIActivityViewController @end

@interface UIDocumentPickerViewController : UIViewController @end
@implementation UIDocumentPickerViewController @end

@interface UIImagePickerController : UINavigationController @end
@implementation UIImagePickerController @end

@interface UIDocument : NSObject @end
@implementation UIDocument @end

@interface UIPopoverController : NSObject @end
@implementation UIPopoverController @end

@interface UIRuntimeConnection : NSObject @end
@implementation UIRuntimeConnection @end

@interface UIRuntimeOutletConnection : UIRuntimeConnection @end
@implementation UIRuntimeOutletConnection @end

@interface UIRuntimeOutletCollectionConnection : UIRuntimeConnection @end
@implementation UIRuntimeOutletCollectionConnection @end

@interface UIRuntimeEventConnection : UIRuntimeConnection @end
@implementation UIRuntimeEventConnection @end

#pragma clang diagnostic pop
