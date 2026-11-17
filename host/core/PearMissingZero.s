// Zero result for any return convention that fits in registers (AAPCS64):
// integers/pointers in x0-x1, floating point and HFA structs (CGRect,
// CGPoint, CGSize, UIEdgeInsets) in d0-d3.
    .text
    .p2align 2
    .globl _pear_missing_zero
_pear_missing_zero:
    mov x0, #0
    mov x1, #0
    movi d0, #0
    movi d1, #0
    movi d2, #0
    movi d3, #0
    ret
