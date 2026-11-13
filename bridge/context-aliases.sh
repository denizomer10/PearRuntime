# Export public ABI names as symbol aliases, retaining original ObjC class names.
PEAR_EAGL_ALIAS_FLAGS=(
  '-Wl,-alias,_OBJC_CLASS_$_PearEAGLContext,_OBJC_CLASS_$_EAGLContext'
  '-Wl,-alias,_OBJC_METACLASS_$_PearEAGLContext,_OBJC_METACLASS_$_EAGLContext'
  '-Wl,-alias,_OBJC_CLASS_$_PearEAGLSharegroup,_OBJC_CLASS_$_EAGLSharegroup'
  '-Wl,-alias,_OBJC_METACLASS_$_PearEAGLSharegroup,_OBJC_METACLASS_$_EAGLSharegroup'
)
