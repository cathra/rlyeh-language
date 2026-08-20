; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.fmt.0 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"

define i32 @main() {
entry:
  %__tmp0.addr = alloca i8*
  %__tmp1.addr = alloca i8*
  %__tmp2.addr = alloca i8*
  %__tmp3.addr = alloca i8*
  %__tmp4.addr = alloca i8*
  %_t0.addr = alloca i8*
  %_t1.addr = alloca i8*
  %_t10.addr = alloca i64
  %_t12.addr = alloca i8*
  %_t13.addr = alloca i8*
  %_t14.addr = alloca i64
  %_t15.addr = alloca i64
  %_t16.addr = alloca i64
  %_t17.addr = alloca i8*
  %_t18.addr = alloca i64
  %_t19.addr = alloca i64
  %_t2.addr = alloca i64
  %_t20.addr = alloca i8*
  %_t21.addr = alloca i64
  %_t22.addr = alloca i64
  %_t23.addr = alloca i8*
  %_t24.addr = alloca i64
  %_t25.addr = alloca i64
  %_t26.addr = alloca i8*
  %_t27.addr = alloca i64
  %_t28.addr = alloca i64
  %_t3.addr = alloca i64
  %_t31.addr = alloca i64
  %_t4.addr = alloca i8*
  %_t5.addr = alloca i64
  %_t6.addr = alloca i64
  %_t7.addr = alloca i64
  %_t8.addr = alloca i8*
  %_t9.addr = alloca i64
  %g.addr = alloca i8*
  %m.addr = alloca i8*
  %t.addr = alloca i64
  %v.addr = alloca i64
  %r0 = call i8* @malloc(i64 16)
  store i8* %r0, i8** %_t0.addr
  %r1 = load i8*, i8** %_t0.addr
  store i8* %r1, i8** %__tmp2.addr
  %r2 = call i8* @malloc(i64 16)
  store i8* %r2, i8** %_t1.addr
  %r3 = load i8*, i8** %_t1.addr
  store i8* %r3, i8** %__tmp0.addr
  store i64 1, i64* %_t2.addr
  %r4 = load i8*, i8** %__tmp0.addr
  %r5 = getelementptr i8, i8* %r4, i64 0
  %r6 = bitcast i8* %r5 to i64*
  %r7 = load i64, i64* %_t2.addr
  store i64 %r7, i64* %r6
  store i64 2, i64* %_t3.addr
  %r8 = load i8*, i8** %__tmp0.addr
  %r9 = getelementptr i8, i8* %r8, i64 8
  %r10 = bitcast i8* %r9 to i64*
  %r11 = load i64, i64* %_t3.addr
  store i64 %r11, i64* %r10
  %r12 = load i8*, i8** %__tmp2.addr
  %r13 = getelementptr i8, i8* %r12, i64 0
  %r14 = bitcast i8* %r13 to i8**
  %r15 = load i8*, i8** %__tmp0.addr
  store i8* %r15, i8** %r14
  %r16 = call i8* @malloc(i64 16)
  store i8* %r16, i8** %_t4.addr
  %r17 = load i8*, i8** %_t4.addr
  store i8* %r17, i8** %__tmp1.addr
  store i64 3, i64* %_t5.addr
  %r18 = load i8*, i8** %__tmp1.addr
  %r19 = getelementptr i8, i8* %r18, i64 0
  %r20 = bitcast i8* %r19 to i64*
  %r21 = load i64, i64* %_t5.addr
  store i64 %r21, i64* %r20
  store i64 4, i64* %_t6.addr
  %r22 = load i8*, i8** %__tmp1.addr
  %r23 = getelementptr i8, i8* %r22, i64 8
  %r24 = bitcast i8* %r23 to i64*
  %r25 = load i64, i64* %_t6.addr
  store i64 %r25, i64* %r24
  %r26 = load i8*, i8** %__tmp2.addr
  %r27 = getelementptr i8, i8* %r26, i64 8
  %r28 = bitcast i8* %r27 to i8**
  %r29 = load i8*, i8** %__tmp1.addr
  store i8* %r29, i8** %r28
  %r30 = load i8*, i8** %__tmp2.addr
  store i8* %r30, i8** %m.addr
  store i64 1, i64* %_t7.addr
  %r31 = load i8*, i8** %m.addr
  %r32 = load i64, i64* %_t7.addr
  %r34 = mul i64 %r32, 8
  %r33 = getelementptr i8, i8* %r31, i64 %r34
  %r35 = bitcast i8* %r33 to i8**
  %r36 = load i8*, i8** %r35
  store i8* %r36, i8** %_t8.addr
  store i64 0, i64* %_t9.addr
  %r37 = load i8*, i8** %_t8.addr
  %r38 = load i64, i64* %_t9.addr
  %r40 = mul i64 %r38, 8
  %r39 = getelementptr i8, i8* %r37, i64 %r40
  %r41 = bitcast i8* %r39 to i64*
  %r42 = load i64, i64* %r41
  store i64 %r42, i64* %_t10.addr
  %r43 = load i64, i64* %_t10.addr
  store i64 %r43, i64* %v.addr
  %r44 = load i64, i64* %v.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.0, i64 0, i64 0), i64 %r44)
  %r45 = call i8* @malloc(i64 8)
  store i8* %r45, i8** %_t12.addr
  %r46 = load i8*, i8** %_t12.addr
  store i8* %r46, i8** %__tmp3.addr
  %r47 = call i8* @malloc(i64 24)
  store i8* %r47, i8** %_t13.addr
  %r48 = load i8*, i8** %_t13.addr
  store i8* %r48, i8** %__tmp4.addr
  store i64 7, i64* %_t14.addr
  %r49 = load i8*, i8** %__tmp4.addr
  %r50 = getelementptr i8, i8* %r49, i64 0
  %r51 = bitcast i8* %r50 to i64*
  %r52 = load i64, i64* %_t14.addr
  store i64 %r52, i64* %r51
  store i64 8, i64* %_t15.addr
  %r53 = load i8*, i8** %__tmp4.addr
  %r54 = getelementptr i8, i8* %r53, i64 8
  %r55 = bitcast i8* %r54 to i64*
  %r56 = load i64, i64* %_t15.addr
  store i64 %r56, i64* %r55
  store i64 9, i64* %_t16.addr
  %r57 = load i8*, i8** %__tmp4.addr
  %r58 = getelementptr i8, i8* %r57, i64 16
  %r59 = bitcast i8* %r58 to i64*
  %r60 = load i64, i64* %_t16.addr
  store i64 %r60, i64* %r59
  %r61 = load i8*, i8** %__tmp3.addr
  %r62 = getelementptr i8, i8* %r61, i64 0
  %r63 = bitcast i8* %r62 to i8**
  %r64 = load i8*, i8** %__tmp4.addr
  store i8* %r64, i8** %r63
  %r65 = load i8*, i8** %__tmp3.addr
  store i8* %r65, i8** %g.addr
  %r66 = load i8*, i8** %g.addr
  %r67 = getelementptr i8, i8* %r66, i64 0
  %r68 = bitcast i8* %r67 to i8**
  %r69 = load i8*, i8** %r68
  store i8* %r69, i8** %_t17.addr
  store i64 1, i64* %_t18.addr
  store i64 99, i64* %_t19.addr
  %r70 = load i8*, i8** %_t17.addr
  %r71 = load i64, i64* %_t18.addr
  %r73 = mul i64 %r71, 8
  %r72 = getelementptr i8, i8* %r70, i64 %r73
  %r74 = bitcast i8* %r72 to i64*
  %r75 = load i64, i64* %_t19.addr
  store i64 %r75, i64* %r74
  %r76 = load i8*, i8** %g.addr
  %r77 = getelementptr i8, i8* %r76, i64 0
  %r78 = bitcast i8* %r77 to i8**
  %r79 = load i8*, i8** %r78
  store i8* %r79, i8** %_t20.addr
  store i64 0, i64* %_t21.addr
  %r80 = load i8*, i8** %_t20.addr
  %r81 = load i64, i64* %_t21.addr
  %r83 = mul i64 %r81, 8
  %r82 = getelementptr i8, i8* %r80, i64 %r83
  %r84 = bitcast i8* %r82 to i64*
  %r85 = load i64, i64* %r84
  store i64 %r85, i64* %_t22.addr
  %r86 = load i8*, i8** %g.addr
  %r87 = getelementptr i8, i8* %r86, i64 0
  %r88 = bitcast i8* %r87 to i8**
  %r89 = load i8*, i8** %r88
  store i8* %r89, i8** %_t23.addr
  store i64 1, i64* %_t24.addr
  %r90 = load i8*, i8** %_t23.addr
  %r91 = load i64, i64* %_t24.addr
  %r93 = mul i64 %r91, 8
  %r92 = getelementptr i8, i8* %r90, i64 %r93
  %r94 = bitcast i8* %r92 to i64*
  %r95 = load i64, i64* %r94
  store i64 %r95, i64* %_t25.addr
  %r96 = load i8*, i8** %g.addr
  %r97 = getelementptr i8, i8* %r96, i64 0
  %r98 = bitcast i8* %r97 to i8**
  %r99 = load i8*, i8** %r98
  store i8* %r99, i8** %_t26.addr
  store i64 2, i64* %_t27.addr
  %r100 = load i8*, i8** %_t26.addr
  %r101 = load i64, i64* %_t27.addr
  %r103 = mul i64 %r101, 8
  %r102 = getelementptr i8, i8* %r100, i64 %r103
  %r104 = bitcast i8* %r102 to i64*
  %r105 = load i64, i64* %r104
  store i64 %r105, i64* %_t28.addr
  %r106 = load i64, i64* %_t22.addr
  %r107 = load i64, i64* %_t25.addr
  %r108 = add i64 %r106, %r107
  store i64 %r108, i64* %_t31.addr
  %r109 = load i64, i64* %_t31.addr
  %r110 = load i64, i64* %_t28.addr
  %r111 = add i64 %r109, %r110
  store i64 %r111, i64* %t.addr
  %r112 = load i64, i64* %t.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.1, i64 0, i64 0), i64 %r112)
  ret i32 0
}

