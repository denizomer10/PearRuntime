#pragma once
#import <UIKit/UIKit.h>
// One conversion for native and subclassed synthetic touches. View-local
// answers stay local; fitted GL window-space answers use the visible layer tree.
CGPoint PearGLMapTouchPoint(UITouch *touch, UIView *view, CGPoint point);
