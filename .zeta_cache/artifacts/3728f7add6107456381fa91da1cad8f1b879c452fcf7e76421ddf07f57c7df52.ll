; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.fmt.0 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.str.1 = private unnamed_addr constant [6 x i8] c"hello\00"
@.fmt.2 = private unnamed_addr constant [4 x i8] c"%c\0A\00"

define i32 @main() {
entry:
  %__tmp0.addr = alloca i8*
  %_t0.addr = alloca i8*
  %_t1.addr = alloca i64
  %_t10.addr = alloca i64
  %_t11.addr = alloca i64
  %_t12.addr = alloca i64
  %_t13.addr = alloca i64
  %_t14.addr = alloca i64
  %_t16.addr = alloca i64
  %_t17.addr = alloca i8
  %_t2.addr = alloca i64
  %_t20.addr = alloca i64
  %_t21.addr = alloca i64
  %_t3.addr = alloca i64
  %_t4.addr = alloca i64
  %_t5.addr = alloca i64
  %_t6.addr = alloca i64
  %_t7.addr = alloca i64
  %_t8.addr = alloca i64
  %_t9.addr = alloca i64
  %arr.addr = alloca i8*
  %ch.addr = alloca i8
  %s.addr = alloca i8*
  %total.addr = alloca i64
  %r0 = call i8* @malloc(i64 32)
  store i8* %r0, i8** %_t0.addr
  %r1 = load i8*, i8** %_t0.addr
  store i8* %r1, i8** %__tmp0.addr
  store i64 1, i64* %_t1.addr
  %r2 = load i8*, i8** %__tmp0.addr
  %r3 = getelementptr i8, i8* %r2, i64 0
  %r4 = bitcast i8* %r3 to i64*
  %r5 = load i64, i64* %_t1.addr
  store i64 %r5, i64* %r4
  store i64 2, i64* %_t2.addr
  %r6 = load i8*, i8** %__tmp0.addr
  %r7 = getelementptr i8, i8* %r6, i64 8
  %r8 = bitcast i8* %r7 to i64*
  %r9 = load i64, i64* %_t2.addr
  store i64 %r9, i64* %r8
  store i64 3, i64* %_t3.addr
  %r10 = load i8*, i8** %__tmp0.addr
  %r11 = getelementptr i8, i8* %r10, i64 16
  %r12 = bitcast i8* %r11 to i64*
  %r13 = load i64, i64* %_t3.addr
  store i64 %r13, i64* %r12
  store i64 4, i64* %_t4.addr
  %r14 = load i8*, i8** %__tmp0.addr
  %r15 = getelementptr i8, i8* %r14, i64 24
  %r16 = bitcast i8* %r15 to i64*
  %r17 = load i64, i64* %_t4.addr
  store i64 %r17, i64* %r16
  %r18 = load i8*, i8** %__tmp0.addr
  store i8* %r18, i8** %arr.addr
  store i64 1, i64* %_t5.addr
  store i64 99, i64* %_t6.addr
  %r19 = load i8*, i8** %arr.addr
  %r20 = load i64, i64* %_t5.addr
  %r22 = mul i64 %r20, 8
  %r21 = getelementptr i8, i8* %r19, i64 %r22
  %r23 = bitcast i8* %r21 to i64*
  %r24 = load i64, i64* %_t6.addr
  store i64 %r24, i64* %r23
  store i64 0, i64* %_t7.addr
  %r25 = load i8*, i8** %arr.addr
  %r26 = load i64, i64* %_t7.addr
  %r28 = mul i64 %r26, 8
  %r27 = getelementptr i8, i8* %r25, i64 %r28
  %r29 = bitcast i8* %r27 to i64*
  %r30 = load i64, i64* %r29
  store i64 %r30, i64* %_t8.addr
  store i64 1, i64* %_t9.addr
  %r31 = load i8*, i8** %arr.addr
  %r32 = load i64, i64* %_t9.addr
  %r34 = mul i64 %r32, 8
  %r33 = getelementptr i8, i8* %r31, i64 %r34
  %r35 = bitcast i8* %r33 to i64*
  %r36 = load i64, i64* %r35
  store i64 %r36, i64* %_t10.addr
  store i64 2, i64* %_t11.addr
  %r37 = load i8*, i8** %arr.addr
  %r38 = load i64, i64* %_t11.addr
  %r40 = mul i64 %r38, 8
  %r39 = getelementptr i8, i8* %r37, i64 %r40
  %r41 = bitcast i8* %r39 to i64*
  %r42 = load i64, i64* %r41
  store i64 %r42, i64* %_t12.addr
  store i64 3, i64* %_t13.addr
  %r43 = load i8*, i8** %arr.addr
  %r44 = load i64, i64* %_t13.addr
  %r46 = mul i64 %r44, 8
  %r45 = getelementptr i8, i8* %r43, i64 %r46
  %r47 = bitcast i8* %r45 to i64*
  %r48 = load i64, i64* %r47
  store i64 %r48, i64* %_t14.addr
  %r49 = load i64, i64* %_t8.addr
  %r50 = load i64, i64* %_t10.addr
  %r51 = add i64 %r49, %r50
  store i64 %r51, i64* %_t20.addr
  %r52 = load i64, i64* %_t20.addr
  %r53 = load i64, i64* %_t12.addr
  %r54 = add i64 %r52, %r53
  store i64 %r54, i64* %_t21.addr
  %r55 = load i64, i64* %_t21.addr
  %r56 = load i64, i64* %_t14.addr
  %r57 = add i64 %r55, %r56
  store i64 %r57, i64* %total.addr
  %r58 = load i64, i64* %total.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.0, i64 0, i64 0), i64 %r58)
  store i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.str.1, i64 0, i64 0), i8** %s.addr
  store i64 1, i64* %_t16.addr
  %r59 = load i8*, i8** %s.addr
  %r60 = load i64, i64* %_t16.addr
  %r61 = getelementptr i8, i8* %r59, i64 %r60
  %r62 = bitcast i8* %r61 to i8*
  %r63 = load i8, i8* %r62
  store i8 %r63, i8* %_t17.addr
  %r64 = load i8, i8* %_t17.addr
  store i8 %r64, i8* %ch.addr
  %r65 = load i8, i8* %ch.addr
  %r66 = zext i8 %r65 to i32
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.fmt.2, i64 0, i64 0), i32 %r66)
  ret i32 0
}

