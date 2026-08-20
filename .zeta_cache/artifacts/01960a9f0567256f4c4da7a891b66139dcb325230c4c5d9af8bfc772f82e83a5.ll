; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.fmt.0 = private unnamed_addr constant [4 x i8] c"%f\0A\00"
@.fmt.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.2 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"

define i32 @main() {
entry:
  %__tmp0.addr = alloca i8*
  %__tmp2.addr = alloca i8*
  %__tmp4.addr = alloca i8*
  %_t0.addr = alloca i8*
  %_t1.addr = alloca i64
  %_t10.addr = alloca i8*
  %_t11.addr = alloca i64
  %_t12.addr = alloca i64
  %_t13.addr = alloca i64
  %_t2.addr = alloca double
  %_t3.addr = alloca double
  %_t5.addr = alloca i8*
  %_t6.addr = alloca i64
  %_t7.addr = alloca i64
  %_t8.addr = alloca i64
  %a.addr = alloca i8*
  %b.addr = alloca i8*
  %r.addr = alloca i8*
  %r0 = call i8* @malloc(i64 16)
  store i8* %r0, i8** %_t0.addr
  %r1 = load i8*, i8** %_t0.addr
  store i8* %r1, i8** %__tmp0.addr
  store i64 1, i64* %_t1.addr
  %r2 = load i8*, i8** %__tmp0.addr
  %r3 = getelementptr i8, i8* %r2, i64 0
  %r4 = bitcast i8* %r3 to i64*
  %r5 = load i64, i64* %_t1.addr
  store i64 %r5, i64* %r4
  store double 0x400C000000000000, double* %_t2.addr
  %r6 = load i8*, i8** %__tmp0.addr
  %r7 = getelementptr i8, i8* %r6, i64 8
  %r8 = bitcast i8* %r7 to double*
  %r9 = load double, double* %_t2.addr
  store double %r9, double* %r8
  %r10 = load i8*, i8** %__tmp0.addr
  store i8* %r10, i8** %a.addr
  %r11 = load i64, i64* %a.addr
  %r12 = call double @"Option::unwrap__f64"(i64 %r11)
  store double %r12, double* %_t3.addr
  %r13 = load double, double* %_t3.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.fmt.0, i64 0, i64 0), double %r13)
  %r14 = call i8* @malloc(i64 16)
  store i8* %r14, i8** %_t5.addr
  %r15 = load i8*, i8** %_t5.addr
  store i8* %r15, i8** %__tmp2.addr
  store i64 0, i64* %_t6.addr
  %r16 = load i8*, i8** %__tmp2.addr
  %r17 = getelementptr i8, i8* %r16, i64 0
  %r18 = bitcast i8* %r17 to i64*
  %r19 = load i64, i64* %_t6.addr
  store i64 %r19, i64* %r18
  %r20 = load i8*, i8** %__tmp2.addr
  store i8* %r20, i8** %b.addr
  store i64 -1, i64* %_t7.addr
  %r21 = load i64, i64* %b.addr
  %r22 = load i64, i64* %_t7.addr
  %r23 = call i64 @"Option::unwrap_or___"(i64 %r21, i64 %r22)
  store i64 %r23, i64* %_t8.addr
  %r24 = load i64, i64* %_t8.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.1, i64 0, i64 0), i64 %r24)
  %r25 = call i8* @malloc(i64 16)
  store i8* %r25, i8** %_t10.addr
  %r26 = load i8*, i8** %_t10.addr
  store i8* %r26, i8** %__tmp4.addr
  store i64 0, i64* %_t11.addr
  %r27 = load i8*, i8** %__tmp4.addr
  %r28 = getelementptr i8, i8* %r27, i64 0
  %r29 = bitcast i8* %r28 to i64*
  %r30 = load i64, i64* %_t11.addr
  store i64 %r30, i64* %r29
  store i64 9, i64* %_t12.addr
  %r31 = load i8*, i8** %__tmp4.addr
  %r32 = getelementptr i8, i8* %r31, i64 8
  %r33 = bitcast i8* %r32 to i64*
  %r34 = load i64, i64* %_t12.addr
  store i64 %r34, i64* %r33
  %r35 = load i8*, i8** %__tmp4.addr
  store i8* %r35, i8** %r.addr
  %r36 = load i64, i64* %r.addr
  %r37 = call i64 @"Result::unwrap__i64__"(i64 %r36)
  store i64 %r37, i64* %_t13.addr
  %r38 = load i64, i64* %_t13.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.2, i64 0, i64 0), i64 %r38)
  ret i32 0
}

define double @"Option::unwrap__f64"(i64 %self) {
entry:
  %__tmp1.addr = alloca i64
  %_t0.addr = alloca i64
  %_t1.addr = alloca i1
  %_t2.addr = alloca double
  %_t3.addr = alloca double
  %_t4.addr = alloca i64
  %_t5.addr = alloca i1
  %self.addr = alloca i64
  %v.addr = alloca double
  store i64 %self, i64* %self.addr
  %r39 = load i64, i64* %self.addr
  store i64 %r39, i64* %__tmp1.addr
  %r40 = load i8*, i8** %__tmp1.addr
  %r41 = getelementptr i8, i8* %r40, i64 0
  %r42 = bitcast i8* %r41 to i64*
  %r43 = load i64, i64* %r42
  store i64 %r43, i64* %_t0.addr
  %r44 = load i64, i64* %_t0.addr
  %r45 = icmp eq i64 %r44, 1
  store i1 %r45, i1* %_t1.addr
  %r46 = load i1, i1* %_t1.addr
  br i1 %r46, label %b1, label %b3
b1:
  %r47 = load i8*, i8** %__tmp1.addr
  %r48 = getelementptr i8, i8* %r47, i64 8
  %r49 = bitcast i8* %r48 to double*
  %r50 = load double, double* %r49
  store double %r50, double* %_t3.addr
  %r51 = load double, double* %_t3.addr
  store double %r51, double* %v.addr
  %r52 = load double, double* %v.addr
  store double %r52, double* %_t2.addr
  br label %b2
b2:
  %r53 = load double, double* %_t2.addr
  ret double %r53
b3:
  %r54 = load i8*, i8** %__tmp1.addr
  %r55 = getelementptr i8, i8* %r54, i64 0
  %r56 = bitcast i8* %r55 to i64*
  %r57 = load i64, i64* %r56
  store i64 %r57, i64* %_t4.addr
  %r58 = load i64, i64* %_t4.addr
  %r59 = icmp eq i64 %r58, 0
  store i1 %r59, i1* %_t5.addr
  %r60 = load i1, i1* %_t5.addr
  br i1 %r60, label %b4, label %b5
b4:
  br label %b6
b5:
  br label %b2
b6:
  br label %b6
}

define i64 @"Option::unwrap_or___"(i64 %self, i64 %default) {
entry:
  %__tmp3.addr = alloca i64
  %_t0.addr = alloca i64
  %_t1.addr = alloca i1
  %_t2.addr = alloca i64
  %_t3.addr = alloca i64
  %_t4.addr = alloca i64
  %_t5.addr = alloca i1
  %_t6.addr = alloca i64
  %default.addr = alloca i64
  %self.addr = alloca i64
  %v.addr = alloca i64
  store i64 %self, i64* %self.addr
  store i64 %default, i64* %default.addr
  %r61 = load i64, i64* %self.addr
  store i64 %r61, i64* %__tmp3.addr
  %r62 = load i8*, i8** %__tmp3.addr
  %r63 = getelementptr i8, i8* %r62, i64 0
  %r64 = bitcast i8* %r63 to i64*
  %r65 = load i64, i64* %r64
  store i64 %r65, i64* %_t0.addr
  %r66 = load i64, i64* %_t0.addr
  %r67 = icmp eq i64 %r66, 1
  store i1 %r67, i1* %_t1.addr
  %r68 = load i1, i1* %_t1.addr
  br i1 %r68, label %b1, label %b3
b1:
  %r69 = load i8*, i8** %__tmp3.addr
  %r70 = getelementptr i8, i8* %r69, i64 8
  %r71 = bitcast i8* %r70 to i64*
  %r72 = load i64, i64* %r71
  store i64 %r72, i64* %_t3.addr
  %r73 = load i64, i64* %_t3.addr
  store i64 %r73, i64* %v.addr
  %r74 = load i64, i64* %v.addr
  store i64 %r74, i64* %_t2.addr
  br label %b2
b2:
  %r75 = load i64, i64* %_t2.addr
  ret i64 %r75
b3:
  %r76 = load i8*, i8** %__tmp3.addr
  %r77 = getelementptr i8, i8* %r76, i64 0
  %r78 = bitcast i8* %r77 to i64*
  %r79 = load i64, i64* %r78
  store i64 %r79, i64* %_t4.addr
  %r80 = load i64, i64* %_t4.addr
  %r81 = icmp eq i64 %r80, 0
  store i1 %r81, i1* %_t5.addr
  %r82 = load i1, i1* %_t5.addr
  br i1 %r82, label %b4, label %b5
b4:
  %r83 = load i64, i64* %default.addr
  store i64 %r83, i64* %_t6.addr
  br label %b5
b5:
  %r84 = load i64, i64* %_t6.addr
  store i64 %r84, i64* %_t2.addr
  br label %b2
}

define i64 @"Result::unwrap__i64__"(i64 %self) {
entry:
  %__tmp5.addr = alloca i64
  %_t0.addr = alloca i64
  %_t1.addr = alloca i1
  %_t2.addr = alloca i64
  %_t3.addr = alloca i64
  %_t4.addr = alloca i64
  %_t5.addr = alloca i1
  %_t7.addr = alloca i64
  %e.addr = alloca i64
  %self.addr = alloca i64
  %v.addr = alloca i64
  store i64 %self, i64* %self.addr
  %r85 = load i64, i64* %self.addr
  store i64 %r85, i64* %__tmp5.addr
  %r86 = load i8*, i8** %__tmp5.addr
  %r87 = getelementptr i8, i8* %r86, i64 0
  %r88 = bitcast i8* %r87 to i64*
  %r89 = load i64, i64* %r88
  store i64 %r89, i64* %_t0.addr
  %r90 = load i64, i64* %_t0.addr
  %r91 = icmp eq i64 %r90, 0
  store i1 %r91, i1* %_t1.addr
  %r92 = load i1, i1* %_t1.addr
  br i1 %r92, label %b1, label %b3
b1:
  %r93 = load i8*, i8** %__tmp5.addr
  %r94 = getelementptr i8, i8* %r93, i64 8
  %r95 = bitcast i8* %r94 to i64*
  %r96 = load i64, i64* %r95
  store i64 %r96, i64* %_t3.addr
  %r97 = load i64, i64* %_t3.addr
  store i64 %r97, i64* %v.addr
  %r98 = load i64, i64* %v.addr
  store i64 %r98, i64* %_t2.addr
  br label %b2
b2:
  %r99 = load i64, i64* %_t2.addr
  ret i64 %r99
b3:
  %r100 = load i8*, i8** %__tmp5.addr
  %r101 = getelementptr i8, i8* %r100, i64 0
  %r102 = bitcast i8* %r101 to i64*
  %r103 = load i64, i64* %r102
  store i64 %r103, i64* %_t4.addr
  %r104 = load i64, i64* %_t4.addr
  %r105 = icmp eq i64 %r104, 1
  store i1 %r105, i1* %_t5.addr
  %r106 = load i1, i1* %_t5.addr
  br i1 %r106, label %b4, label %b5
b4:
  %r107 = load i8*, i8** %__tmp5.addr
  %r108 = getelementptr i8, i8* %r107, i64 8
  %r109 = bitcast i8* %r108 to i64*
  %r110 = load i64, i64* %r109
  store i64 %r110, i64* %_t7.addr
  %r111 = load i64, i64* %_t7.addr
  store i64 %r111, i64* %e.addr
  br label %b6
b5:
  br label %b2
b6:
  br label %b6
}

