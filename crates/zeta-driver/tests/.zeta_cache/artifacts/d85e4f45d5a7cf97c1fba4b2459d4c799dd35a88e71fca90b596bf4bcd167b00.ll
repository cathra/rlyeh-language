; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.fmt.0 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.2 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.3 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.4 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.5 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"

define i32 @main() {
entry:
  %None.addr = alloca i8*
  %__tmp0.addr = alloca i8*
  %__tmp1.addr = alloca i8*
  %__tmp2.addr = alloca i8*
  %__tmp3.addr = alloca i8*
  %__tmp4.addr = alloca i8*
  %__tmp5.addr = alloca i8*
  %__tmp6.addr = alloca i8*
  %_t0.addr = alloca i8*
  %_t1.addr = alloca i64
  %_t10.addr = alloca i1
  %_t12.addr = alloca i64
  %_t15.addr = alloca i8*
  %_t16.addr = alloca i64
  %_t17.addr = alloca i64
  %_t18.addr = alloca i1
  %_t2.addr = alloca i64
  %_t20.addr = alloca i64
  %_t22.addr = alloca i64
  %_t23.addr = alloca i1
  %_t25.addr = alloca i64
  %_t28.addr = alloca i64
  %_t3.addr = alloca i64
  %_t31.addr = alloca i8*
  %_t32.addr = alloca i64
  %_t33.addr = alloca i64
  %_t34.addr = alloca i64
  %_t35.addr = alloca i1
  %_t37.addr = alloca i64
  %_t38.addr = alloca i64
  %_t4.addr = alloca i1
  %_t40.addr = alloca i64
  %_t43.addr = alloca i8*
  %_t44.addr = alloca i64
  %_t45.addr = alloca i64
  %_t6.addr = alloca i64
  %_t7.addr = alloca i64
  %_t9.addr = alloca i64
  %c.addr = alloca i64
  %e.addr = alloca i8*
  %r.addr = alloca i64
  %r2.addr = alloca i64
  %r3.addr = alloca i64
  %v.addr = alloca i8*
  %v2.addr = alloca i8*
  %v3.addr = alloca i8*
  %x.addr = alloca i64
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
  store i8* %r10, i8** %v.addr
  store i64 0, i64* %r.addr
  %r11 = load i8*, i8** %v.addr
  store i8* %r11, i8** %__tmp1.addr
  %r12 = load i8*, i8** %__tmp1.addr
  %r13 = getelementptr i8, i8* %r12, i64 0
  %r14 = bitcast i8* %r13 to i64*
  %r15 = load i64, i64* %r14
  store i64 %r15, i64* %_t3.addr
  %r16 = load i64, i64* %_t3.addr
  %r17 = icmp eq i64 %r16, 0
  store i1 %r17, i1* %_t4.addr
  %r18 = load i1, i1* %_t4.addr
  br i1 %r18, label %b1, label %b3
b1:
  %r19 = load i8*, i8** %__tmp1.addr
  %r20 = getelementptr i8, i8* %r19, i64 8
  %r21 = bitcast i8* %r20 to i64*
  %r22 = load i64, i64* %r21
  store i64 %r22, i64* %_t6.addr
  %r23 = load i64, i64* %_t6.addr
  store i64 %r23, i64* %x.addr
  %r24 = load i64, i64* %x.addr
  %r25 = mul i64 %r24, 2
  store i64 %r25, i64* %_t7.addr
  %r26 = load i64, i64* %_t7.addr
  store i64 %r26, i64* %r.addr
  br label %b2
b2:
  %r27 = load i64, i64* %r.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.0, i64 0, i64 0), i64 %r27)
  %r28 = call i8* @malloc(i64 16)
  store i8* %r28, i8** %_t15.addr
  %r29 = load i8*, i8** %_t15.addr
  store i8* %r29, i8** %__tmp2.addr
  store i64 1, i64* %_t16.addr
  %r30 = load i8*, i8** %__tmp2.addr
  %r31 = getelementptr i8, i8* %r30, i64 0
  %r32 = bitcast i8* %r31 to i64*
  %r33 = load i64, i64* %_t16.addr
  store i64 %r33, i64* %r32
  %r34 = load i8*, i8** %__tmp2.addr
  store i8* %r34, i8** %v2.addr
  store i64 0, i64* %r2.addr
  %r35 = load i8*, i8** %v2.addr
  store i8* %r35, i8** %__tmp3.addr
  %r36 = load i8*, i8** %__tmp3.addr
  %r37 = getelementptr i8, i8* %r36, i64 0
  %r38 = bitcast i8* %r37 to i64*
  %r39 = load i64, i64* %r38
  store i64 %r39, i64* %_t17.addr
  %r40 = load i64, i64* %_t17.addr
  %r41 = icmp eq i64 %r40, 0
  store i1 %r41, i1* %_t18.addr
  %r42 = load i1, i1* %_t18.addr
  br i1 %r42, label %b6, label %b8
b3:
  %r43 = load i8*, i8** %__tmp1.addr
  %r44 = getelementptr i8, i8* %r43, i64 0
  %r45 = bitcast i8* %r44 to i64*
  %r46 = load i64, i64* %r45
  store i64 %r46, i64* %_t9.addr
  %r47 = load i64, i64* %_t9.addr
  %r48 = icmp eq i64 %r47, 1
  store i1 %r48, i1* %_t10.addr
  %r49 = load i1, i1* %_t10.addr
  br i1 %r49, label %b4, label %b5
b4:
  store i64 -1, i64* %_t12.addr
  %r50 = load i64, i64* %_t12.addr
  store i64 %r50, i64* %r.addr
  br label %b5
b5:
  br label %b2
b6:
  %r51 = load i8*, i8** %__tmp3.addr
  %r52 = getelementptr i8, i8* %r51, i64 8
  %r53 = bitcast i8* %r52 to i64*
  %r54 = load i64, i64* %r53
  store i64 %r54, i64* %_t20.addr
  %r55 = load i64, i64* %_t20.addr
  store i64 %r55, i64* %x.addr
  %r56 = load i64, i64* %x.addr
  store i64 %r56, i64* %r2.addr
  br label %b7
b7:
  %r57 = load i64, i64* %r2.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.1, i64 0, i64 0), i64 %r57)
  store i64 3, i64* %_t28.addr
  %r58 = load i64, i64* %_t28.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.2, i64 0, i64 0), i64 %r58)
  store i64 30, i64* %c.addr
  %r59 = load i64, i64* %c.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.3, i64 0, i64 0), i64 %r59)
  %r60 = call i8* @malloc(i64 16)
  store i8* %r60, i8** %_t31.addr
  %r61 = load i8*, i8** %_t31.addr
  store i8* %r61, i8** %__tmp4.addr
  store i64 0, i64* %_t32.addr
  %r62 = load i8*, i8** %__tmp4.addr
  %r63 = getelementptr i8, i8* %r62, i64 0
  %r64 = bitcast i8* %r63 to i64*
  %r65 = load i64, i64* %_t32.addr
  store i64 %r65, i64* %r64
  store i64 7, i64* %_t33.addr
  %r66 = load i8*, i8** %__tmp4.addr
  %r67 = getelementptr i8, i8* %r66, i64 8
  %r68 = bitcast i8* %r67 to i64*
  %r69 = load i64, i64* %_t33.addr
  store i64 %r69, i64* %r68
  %r70 = load i8*, i8** %__tmp4.addr
  store i8* %r70, i8** %v3.addr
  store i64 0, i64* %r3.addr
  %r71 = load i8*, i8** %v3.addr
  store i8* %r71, i8** %__tmp5.addr
  %r72 = load i8*, i8** %__tmp5.addr
  %r73 = getelementptr i8, i8* %r72, i64 0
  %r74 = bitcast i8* %r73 to i64*
  %r75 = load i64, i64* %r74
  store i64 %r75, i64* %_t34.addr
  %r76 = load i64, i64* %_t34.addr
  %r77 = icmp eq i64 %r76, 0
  store i1 %r77, i1* %_t35.addr
  %r78 = load i1, i1* %_t35.addr
  br i1 %r78, label %b11, label %b13
b8:
  %r79 = load i8*, i8** %__tmp3.addr
  %r80 = getelementptr i8, i8* %r79, i64 0
  %r81 = bitcast i8* %r80 to i64*
  %r82 = load i64, i64* %r81
  store i64 %r82, i64* %_t22.addr
  %r83 = load i64, i64* %_t22.addr
  %r84 = icmp eq i64 %r83, 1
  store i1 %r84, i1* %_t23.addr
  %r85 = load i1, i1* %_t23.addr
  br i1 %r85, label %b9, label %b10
b9:
  store i64 42, i64* %_t25.addr
  %r86 = load i64, i64* %_t25.addr
  store i64 %r86, i64* %r2.addr
  br label %b10
b10:
  br label %b7
b11:
  %r87 = load i8*, i8** %__tmp5.addr
  %r88 = getelementptr i8, i8* %r87, i64 8
  %r89 = bitcast i8* %r88 to i64*
  %r90 = load i64, i64* %r89
  store i64 %r90, i64* %_t37.addr
  %r91 = load i64, i64* %_t37.addr
  store i64 %r91, i64* %x.addr
  %r92 = load i64, i64* %x.addr
  %r93 = add i64 %r92, 1
  store i64 %r93, i64* %_t38.addr
  %r94 = load i64, i64* %_t38.addr
  store i64 %r94, i64* %r3.addr
  br label %b12
b12:
  %r95 = load i64, i64* %r3.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.4, i64 0, i64 0), i64 %r95)
  %r96 = call i8* @malloc(i64 16)
  store i8* %r96, i8** %_t43.addr
  %r97 = load i8*, i8** %_t43.addr
  store i8* %r97, i8** %__tmp6.addr
  store i64 1, i64* %_t44.addr
  %r98 = load i8*, i8** %__tmp6.addr
  %r99 = getelementptr i8, i8* %r98, i64 0
  %r100 = bitcast i8* %r99 to i64*
  %r101 = load i64, i64* %_t44.addr
  store i64 %r101, i64* %r100
  %r102 = load i8*, i8** %__tmp6.addr
  store i8* %r102, i8** %e.addr
  store i64 99, i64* %_t45.addr
  %r103 = load i64, i64* %_t45.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.5, i64 0, i64 0), i64 %r103)
  ret i32 0
b13:
  %r104 = load i8*, i8** %__tmp5.addr
  store i8* %r104, i8** %None.addr
  store i64 0, i64* %_t40.addr
  %r105 = load i64, i64* %_t40.addr
  store i64 %r105, i64* %r3.addr
  br label %b12
}

