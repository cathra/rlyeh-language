; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.fmt.0 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"

define i32 @main() {
entry:
  %__tmp0.addr = alloca i8*
  %__tmp1.addr = alloca i8*
  %_t0.addr = alloca i8*
  %_t1.addr = alloca i64
  %_t2.addr = alloca i64
  %_t3.addr = alloca i8*
  %_t4.addr = alloca i64
  %_t5.addr = alloca i8*
  %_t6.addr = alloca i64
  %inner.addr = alloca i8*
  %outer.addr = alloca i8*
  %v.addr = alloca i8*
  %r0 = call i8* @malloc(i64 16)
  store i8* %r0, i8** %_t0.addr
  %r1 = load i8*, i8** %_t0.addr
  store i8* %r1, i8** %__tmp0.addr
  store i64 0, i64* %_t1.addr
  %r2 = load i8*, i8** %__tmp0.addr
  %r3 = getelementptr i8, i8* %r2, i64 0
  %r4 = bitcast i8* %r3 to i64*
  %r5 = load i64, i64* %_t1.addr
  store i64 %r5, i64* %r4
  store i64 5, i64* %_t2.addr
  %r6 = load i8*, i8** %__tmp0.addr
  %r7 = getelementptr i8, i8* %r6, i64 8
  %r8 = bitcast i8* %r7 to i64*
  %r9 = load i64, i64* %_t2.addr
  store i64 %r9, i64* %r8
  %r10 = load i8*, i8** %__tmp0.addr
  store i8* %r10, i8** %inner.addr
  %r11 = call i8* @malloc(i64 16)
  store i8* %r11, i8** %_t3.addr
  %r12 = load i8*, i8** %_t3.addr
  store i8* %r12, i8** %__tmp1.addr
  store i64 1, i64* %_t4.addr
  %r13 = load i8*, i8** %__tmp1.addr
  %r14 = getelementptr i8, i8* %r13, i64 0
  %r15 = bitcast i8* %r14 to i64*
  %r16 = load i64, i64* %_t4.addr
  store i64 %r16, i64* %r15
  %r17 = load i8*, i8** %__tmp1.addr
  %r18 = getelementptr i8, i8* %r17, i64 8
  %r19 = bitcast i8* %r18 to i8**
  %r20 = load i8*, i8** %inner.addr
  store i8* %r20, i8** %r19
  %r21 = load i8*, i8** %__tmp1.addr
  store i8* %r21, i8** %outer.addr
  %r22 = load i64, i64* %outer.addr
  %r23 = call i8* @"Option::unwrap__Result_i64__"(i64 %r22)
  store i8* %r23, i8** %_t5.addr
  %r24 = load i8*, i8** %_t5.addr
  store i8* %r24, i8** %v.addr
  %r25 = load i64, i64* %v.addr
  %r26 = call i64 @"Result::is_ok__i64__"(i64 %r25)
  store i64 %r26, i64* %_t6.addr
  %r27 = load i64, i64* %_t6.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.0, i64 0, i64 0), i64 %r27)
  ret i32 0
}

define i8* @"Option::unwrap__Result_i64__"(i64 %self) {
entry:
  %__tmp2.addr = alloca i64
  %_t0.addr = alloca i64
  %_t1.addr = alloca i1
  %_t2.addr = alloca i8*
  %_t3.addr = alloca i8*
  %_t4.addr = alloca i64
  %_t5.addr = alloca i1
  %self.addr = alloca i64
  %v.addr = alloca i8*
  store i64 %self, i64* %self.addr
  %r28 = load i64, i64* %self.addr
  store i64 %r28, i64* %__tmp2.addr
  %r29 = load i8*, i8** %__tmp2.addr
  %r30 = getelementptr i8, i8* %r29, i64 0
  %r31 = bitcast i8* %r30 to i64*
  %r32 = load i64, i64* %r31
  store i64 %r32, i64* %_t0.addr
  %r33 = load i64, i64* %_t0.addr
  %r34 = icmp eq i64 %r33, 1
  store i1 %r34, i1* %_t1.addr
  %r35 = load i1, i1* %_t1.addr
  br i1 %r35, label %b1, label %b3
b1:
  %r36 = load i8*, i8** %__tmp2.addr
  %r37 = getelementptr i8, i8* %r36, i64 8
  %r38 = bitcast i8* %r37 to i8**
  %r39 = load i8*, i8** %r38
  store i8* %r39, i8** %_t3.addr
  %r40 = load i8*, i8** %_t3.addr
  store i8* %r40, i8** %v.addr
  %r41 = load i8*, i8** %v.addr
  store i8* %r41, i8** %_t2.addr
  br label %b2
b2:
  %r42 = load i8*, i8** %_t2.addr
  ret i8* %r42
b3:
  %r43 = load i8*, i8** %__tmp2.addr
  %r44 = getelementptr i8, i8* %r43, i64 0
  %r45 = bitcast i8* %r44 to i64*
  %r46 = load i64, i64* %r45
  store i64 %r46, i64* %_t4.addr
  %r47 = load i64, i64* %_t4.addr
  %r48 = icmp eq i64 %r47, 0
  store i1 %r48, i1* %_t5.addr
  %r49 = load i1, i1* %_t5.addr
  br i1 %r49, label %b4, label %b5
b4:
  br label %b6
b5:
  br label %b2
b6:
  br label %b6
}

define i64 @"Result::is_ok__i64__"(i64 %self) {
entry:
  %__tmp3.addr = alloca i64
  %_t0.addr = alloca i64
  %_t1.addr = alloca i1
  %_t2.addr = alloca i64
  %_t3.addr = alloca i64
  %_t4.addr = alloca i64
  %_t5.addr = alloca i64
  %_t6.addr = alloca i1
  %_t7.addr = alloca i64
  %_t8.addr = alloca i64
  %_t9.addr = alloca i64
  %e.addr = alloca i64
  %self.addr = alloca i64
  %v.addr = alloca i64
  store i64 %self, i64* %self.addr
  %r50 = load i64, i64* %self.addr
  store i64 %r50, i64* %__tmp3.addr
  %r51 = load i8*, i8** %__tmp3.addr
  %r52 = getelementptr i8, i8* %r51, i64 0
  %r53 = bitcast i8* %r52 to i64*
  %r54 = load i64, i64* %r53
  store i64 %r54, i64* %_t0.addr
  %r55 = load i64, i64* %_t0.addr
  %r56 = icmp eq i64 %r55, 0
  store i1 %r56, i1* %_t1.addr
  %r57 = load i1, i1* %_t1.addr
  br i1 %r57, label %b1, label %b3
b1:
  %r58 = load i8*, i8** %__tmp3.addr
  %r59 = getelementptr i8, i8* %r58, i64 8
  %r60 = bitcast i8* %r59 to i64*
  %r61 = load i64, i64* %r60
  store i64 %r61, i64* %_t3.addr
  %r62 = load i64, i64* %_t3.addr
  store i64 %r62, i64* %v.addr
  store i64 1, i64* %_t4.addr
  %r63 = load i64, i64* %_t4.addr
  store i64 %r63, i64* %_t2.addr
  br label %b2
b2:
  %r64 = load i64, i64* %_t2.addr
  ret i64 %r64
b3:
  %r65 = load i8*, i8** %__tmp3.addr
  %r66 = getelementptr i8, i8* %r65, i64 0
  %r67 = bitcast i8* %r66 to i64*
  %r68 = load i64, i64* %r67
  store i64 %r68, i64* %_t5.addr
  %r69 = load i64, i64* %_t5.addr
  %r70 = icmp eq i64 %r69, 1
  store i1 %r70, i1* %_t6.addr
  %r71 = load i1, i1* %_t6.addr
  br i1 %r71, label %b4, label %b5
b4:
  %r72 = load i8*, i8** %__tmp3.addr
  %r73 = getelementptr i8, i8* %r72, i64 8
  %r74 = bitcast i8* %r73 to i64*
  %r75 = load i64, i64* %r74
  store i64 %r75, i64* %_t8.addr
  %r76 = load i64, i64* %_t8.addr
  store i64 %r76, i64* %e.addr
  store i64 0, i64* %_t9.addr
  %r77 = load i64, i64* %_t9.addr
  store i64 %r77, i64* %_t7.addr
  br label %b5
b5:
  %r78 = load i64, i64* %_t7.addr
  store i64 %r78, i64* %_t2.addr
  br label %b2
}

