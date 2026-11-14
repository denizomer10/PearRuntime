// UIAlertController / UIAlertAction, presented as a native alert by the backend.
// Clean-room: from the public UIAlertController documentation.
#import "PearUIKitPrivate.h"

@implementation UIAlertAction {
    NSString *_title;
    NSInteger _style;
    void (^_handler)(UIAlertAction *);
    BOOL _enabled;
}
+ (instancetype)actionWithTitle:(NSString *)title style:(NSInteger)style handler:(void (^)(UIAlertAction *))handler {
    UIAlertAction *a = [self new];
    a->_title = [title copy];
    a->_style = style;
    a->_handler = [handler copy];
    a->_enabled = YES;
    return a;
}
- (NSString *)title { return _title; }
- (NSInteger)style { return _style; }
- (BOOL)isEnabled { return _enabled; }
- (void)setEnabled:(BOOL)e { _enabled = e; }
- (void)pear_fire { if (_handler) _handler(self); }
- (id)copyWithZone:(NSZone *)zone { return self; }
@end

@implementation UIAlertController {
    NSString *_message;
    NSInteger _preferredStyle;
    NSMutableArray<UIAlertAction *> *_actions;
    UIAlertAction *_preferredAction;
}
+ (instancetype)alertControllerWithTitle:(NSString *)title message:(NSString *)message preferredStyle:(NSInteger)style {
    UIAlertController *c = [self new];
    c.title = title;
    c->_message = [message copy];
    c->_preferredStyle = style;
    c->_actions = [NSMutableArray array];
    return c;
}
- (NSString *)message { return _message; }
- (void)setMessage:(NSString *)m { _message = [m copy]; }
- (NSInteger)preferredStyle { return _preferredStyle; }
- (NSArray<UIAlertAction *> *)actions { return [_actions copy]; }
- (void)addAction:(UIAlertAction *)a { if (a) [_actions addObject:a]; }
- (UIAlertAction *)preferredAction { return _preferredAction; }
- (void)setPreferredAction:(UIAlertAction *)a { _preferredAction = a; }
- (NSArray *)textFields { return @[]; }
- (void)addTextFieldWithConfigurationHandler:(void (^)(id))handler {
    PearMissingReport(PearMissingBehavior, "UIAlertController", "text fields in alerts", __builtin_return_address(0));
}
- (id)popoverPresentationController { return nil; }

- (void)pear_presentWithCompletion:(void (^)(void))completion {
    NSMutableArray *titles = [NSMutableArray array];
    for (UIAlertAction *a in _actions) [titles addObject:a.title ?: @"OK"];
    PearHostLog("alert", "title=\"%s\" message=\"%s\" actions=[%s]", self.title.UTF8String ?: "",
                _message.UTF8String ?: "", [titles componentsJoinedByString:@", "].UTF8String);
    NSArray *actions = [_actions copy];
    PearBackendPresentAlert(self.title ?: @"", _message ?: @"", titles, ^(NSInteger index) {
        if (index >= 0 && index < (NSInteger)actions.count) [actions[index] pear_fire];
    });
    if (completion) dispatch_async(dispatch_get_main_queue(), completion);
}
@end
