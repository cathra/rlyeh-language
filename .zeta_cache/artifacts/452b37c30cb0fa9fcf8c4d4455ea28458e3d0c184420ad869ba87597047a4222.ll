; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.fmt.0 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"

define i64 @sum_array(i64 %arr) {
entry:
  %__for_hi_1.addr = alloca i64
  %__for_lo_0.addr = alloca i64
  %_t0.addr = alloca i64
  %_t1.addr = alloca i1
  %_t3.addr = alloca i64
  %arr.addr = alloca i64
  %i.addr = alloca i64
  %s.addr = alloca i64
  store i64 %arr, i64* %arr.addr
  store i64 0, i64* %s.addr
  store i64 0, i64* %__for_lo_0.addr
  store i64 4, i64* %__for_hi_1.addr
  %r0 = load i64, i64* %__for_lo_0.addr
  %r1 = sub i64 %r0, 1
  store i64 %r1, i64* %i.addr
  br label %b1
b1:
  store i64 1, i64* %_t0.addr
  %r2 = load i64, i64* %i.addr
  %r3 = load i64, i64* %_t0.addr
  %r4 = add i64 %r2, %r3
  store i64 %r4, i64* %i.addr
  %r5 = load i64, i64* %i.addr
  %r6 = load i64, i64* %__for_hi_1.addr
  %r7 = icmp sge i64 %r5, %r6
  store i1 %r7, i1* %_t1.addr
  %r8 = load i1, i1* %_t1.addr
  br i1 %r8, label %b3, label %b4
b2:
  %r9 = load i64, i64* %s.addr
  ret i64 %r9
b3:
  br label %b2
b4:
  %r10 = load i8*, i8** %arr.addr
  %r11 = load i64, i64* %i.addr
  %r13 = mul i64 %r11, 8
  %r12 = getelementptr i8, i8* %r10, i64 %r13
  %r14 = bitcast i8* %r12 to i64*
  %r15 = load i64, i64* %r14
  store i64 %r15, i64* %_t3.addr
  %r16 = load i64, i64* %s.addr
  %r17 = load i64, i64* %_t3.addr
  %r18 = add i64 %r16, %r17
  store i64 %r18, i64* %s.addr
  br label %b1
}

define i32 @main() {
entry:
  %__tmp2.addr = alloca i8*
  %_t0.addr = alloca i8*
  %_t1.addr = alloca i64
  %_t2.addr = alloca i64
  %_t3.addr = alloca i64
  %_t4.addr = alloca i64
  %_t5.addr = alloca i64
  %_t6.addr = alloca i64
  %_t7.addr = alloca i64
  %_t9.addr = alloca i64
  %arr.addr = alloca i8*
  %b.addr = alloca i8*
  %i.addr = alloca i64
  %t.addr = alloca i64
  %v.addr = alloca i64
  %r19 = call i8* @malloc(i64 32)
  store i8* %r19, i8** %_t0.addr
  %r20 = load i8*, i8** %_t0.addr
  store i8* %r20, i8** %__tmp2.addr
  store i64 1, i64* %_t1.addr
  %r21 = load i8*, i8** %__tmp2.addr
  %r22 = getelementptr i8, i8* %r21, i64 0
  %r23 = bitcast i8* %r22 to i64*
  %r24 = load i64, i64* %_t1.addr
  store i64 %r24, i64* %r23
  store i64 2, i64* %_t2.addr
  %r25 = load i8*, i8** %__tmp2.addr
  %r26 = getelementptr i8, i8* %r25, i64 8
  %r27 = bitcast i8* %r26 to i64*
  %r28 = load i64, i64* %_t2.addr
  store i64 %r28, i64* %r27
  store i64 3, i64* %_t3.addr
  %r29 = load i8*, i8** %__tmp2.addr
  %r30 = getelementptr i8, i8* %r29, i64 16
  %r31 = bitcast i8* %r30 to i64*
  %r32 = load i64, i64* %_t3.addr
  store i64 %r32, i64* %r31
  store i64 4, i64* %_t4.addr
  %r33 = load i8*, i8** %__tmp2.addr
  %r34 = getelementptr i8, i8* %r33, i64 24
  %r35 = bitcast i8* %r34 to i64*
  %r36 = load i64, i64* %_t4.addr
  store i64 %r36, i64* %r35
  %r37 = load i8*, i8** %__tmp2.addr
  store i8* %r37, i8** %arr.addr
  %r38 = load i8*, i8** %arr.addr
  store i8* %r38, i8** %b.addr
  store i64 0, i64* %_t5.addr
  store i64 10, i64* %_t6.addr
  %r39 = load i8*, i8** %b.addr
  %r40 = load i64, i64* %_t5.addr
  %r42 = mul i64 %r40, 8
  %r41 = getelementptr i8, i8* %r39, i64 %r42
  %r43 = bitcast i8* %r41 to i64*
  %r44 = load i64, i64* %_t6.addr
  store i64 %r44, i64* %r43
  %r45 = load i64, i64* %arr.addr
  %r46 = call i64 @sum_array(i64 %r45)
  store i64 %r46, i64* %_t7.addr
  %r47 = load i64, i64* %_t7.addr
  store i64 %r47, i64* %t.addr
  %r48 = load i64, i64* %t.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.0, i64 0, i64 0), i64 %r48)
  store i64 1, i64* %i.addr
  %r49 = load i8*, i8** %arr.addr
  %r50 = load i64, i64* %i.addr
  %r52 = mul i64 %r50, 8
  %r51 = getelementptr i8, i8* %r49, i64 %r52
  %r53 = bitcast i8* %r51 to i64*
  %r54 = load i64, i64* %r53
  store i64 %r54, i64* %_t9.addr
  %r55 = load i64, i64* %_t9.addr
  store i64 %r55, i64* %v.addr
  %r56 = load i64, i64* %v.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.1, i64 0, i64 0), i64 %r56)
  ret i32 0
}

