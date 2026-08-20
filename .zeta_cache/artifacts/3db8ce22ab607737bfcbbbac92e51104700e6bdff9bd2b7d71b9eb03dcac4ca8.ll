; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.fmt.0 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"

define i32 @main() {
entry:
  %__tmp0.addr = alloca i8*
  %_t0.addr = alloca i8*
  %_t1.addr = alloca i64
  %_t12.addr = alloca i64
  %_t2.addr = alloca i64
  %_t3.addr = alloca i64
  %_t4.addr = alloca i64
  %_t5.addr = alloca i64
  %_t6.addr = alloca i64
  %_t7.addr = alloca i64
  %_t8.addr = alloca i64
  %_t9.addr = alloca i64
  %a.addr = alloca i64
  %arr.addr = alloca i8*
  %b.addr = alloca i64
  %c.addr = alloca i64
  %sum.addr = alloca i64
  %r0 = call i8* @malloc(i64 24)
  store i8* %r0, i8** %_t0.addr
  %r1 = load i8*, i8** %_t0.addr
  store i8* %r1, i8** %__tmp0.addr
  store i64 10, i64* %_t1.addr
  %r2 = load i8*, i8** %__tmp0.addr
  %r3 = getelementptr i8, i8* %r2, i64 0
  %r4 = bitcast i8* %r3 to i64*
  %r5 = load i64, i64* %_t1.addr
  store i64 %r5, i64* %r4
  store i64 20, i64* %_t2.addr
  %r6 = load i8*, i8** %__tmp0.addr
  %r7 = getelementptr i8, i8* %r6, i64 8
  %r8 = bitcast i8* %r7 to i64*
  %r9 = load i64, i64* %_t2.addr
  store i64 %r9, i64* %r8
  store i64 30, i64* %_t3.addr
  %r10 = load i8*, i8** %__tmp0.addr
  %r11 = getelementptr i8, i8* %r10, i64 16
  %r12 = bitcast i8* %r11 to i64*
  %r13 = load i64, i64* %_t3.addr
  store i64 %r13, i64* %r12
  %r14 = load i8*, i8** %__tmp0.addr
  store i8* %r14, i8** %arr.addr
  store i64 0, i64* %_t4.addr
  %r15 = load i8*, i8** %arr.addr
  %r16 = load i64, i64* %_t4.addr
  %r18 = mul i64 %r16, 8
  %r17 = getelementptr i8, i8* %r15, i64 %r18
  %r19 = bitcast i8* %r17 to i64*
  %r20 = load i64, i64* %r19
  store i64 %r20, i64* %_t5.addr
  %r21 = load i64, i64* %_t5.addr
  store i64 %r21, i64* %a.addr
  store i64 1, i64* %_t6.addr
  %r22 = load i8*, i8** %arr.addr
  %r23 = load i64, i64* %_t6.addr
  %r25 = mul i64 %r23, 8
  %r24 = getelementptr i8, i8* %r22, i64 %r25
  %r26 = bitcast i8* %r24 to i64*
  %r27 = load i64, i64* %r26
  store i64 %r27, i64* %_t7.addr
  %r28 = load i64, i64* %_t7.addr
  store i64 %r28, i64* %b.addr
  store i64 2, i64* %_t8.addr
  %r29 = load i8*, i8** %arr.addr
  %r30 = load i64, i64* %_t8.addr
  %r32 = mul i64 %r30, 8
  %r31 = getelementptr i8, i8* %r29, i64 %r32
  %r33 = bitcast i8* %r31 to i64*
  %r34 = load i64, i64* %r33
  store i64 %r34, i64* %_t9.addr
  %r35 = load i64, i64* %_t9.addr
  store i64 %r35, i64* %c.addr
  %r36 = load i64, i64* %a.addr
  %r37 = load i64, i64* %b.addr
  %r38 = add i64 %r36, %r37
  store i64 %r38, i64* %_t12.addr
  %r39 = load i64, i64* %_t12.addr
  %r40 = load i64, i64* %c.addr
  %r41 = add i64 %r39, %r40
  store i64 %r41, i64* %sum.addr
  %r42 = load i64, i64* %sum.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.0, i64 0, i64 0), i64 %r42)
  ret i32 0
}

