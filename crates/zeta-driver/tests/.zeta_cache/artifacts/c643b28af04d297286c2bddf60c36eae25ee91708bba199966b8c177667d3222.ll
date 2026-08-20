; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.fmt.0 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.2 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt.3 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"

define i64 @"shape::make"(i64 %x) {
entry:
  %_t0.addr = alloca i64
  %x.addr = alloca i64
  store i64 %x, i64* %x.addr
  %r0 = load i64, i64* %x.addr
  %r1 = mul i64 %r0, 2
  store i64 %r1, i64* %_t0.addr
  %r2 = load i64, i64* %_t0.addr
  ret i64 %r2
}

define i32 @main() {
entry:
  %__tmp0.addr = alloca i8*
  %__tmp1.addr = alloca i8*
  %_i0.addr = alloca i64
  %_t0.addr = alloca i8*
  %_t1.addr = alloca i64
  %_t11.addr = alloca i64
  %_t12.addr = alloca i64
  %_t2.addr = alloca i64
  %_t4.addr = alloca i8*
  %_t5.addr = alloca i64
  %_t6.addr = alloca i64
  %_t7.addr = alloca i64
  %_t8.addr = alloca i64
  %a.addr = alloca i8*
  %b.addr = alloca i8*
  %c.addr = alloca i64
  %d.addr = alloca i64
  %r3 = call i8* @malloc(i64 24)
  store i8* %r3, i8** %_t0.addr
  %r4 = load i8*, i8** %_t0.addr
  store i8* %r4, i8** %__tmp0.addr
  store i64 0, i64* %_t1.addr
  %r5 = load i8*, i8** %__tmp0.addr
  %r6 = getelementptr i8, i8* %r5, i64 0
  %r7 = bitcast i8* %r6 to i64*
  %r8 = load i64, i64* %_t1.addr
  store i64 %r8, i64* %r7
  %r9 = load i8*, i8** %__tmp0.addr
  store i8* %r9, i8** %a.addr
  store i64 1, i64* %_t2.addr
  %r10 = load i64, i64* %_t2.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.0, i64 0, i64 0), i64 %r10)
  %r11 = call i8* @malloc(i64 24)
  store i8* %r11, i8** %_t4.addr
  %r12 = load i8*, i8** %_t4.addr
  store i8* %r12, i8** %__tmp1.addr
  store i64 1, i64* %_t5.addr
  %r13 = load i8*, i8** %__tmp1.addr
  %r14 = getelementptr i8, i8* %r13, i64 0
  %r15 = bitcast i8* %r14 to i64*
  %r16 = load i64, i64* %_t5.addr
  store i64 %r16, i64* %r15
  store i64 3, i64* %_t6.addr
  %r17 = load i8*, i8** %__tmp1.addr
  %r18 = getelementptr i8, i8* %r17, i64 8
  %r19 = bitcast i8* %r18 to i64*
  %r20 = load i64, i64* %_t6.addr
  store i64 %r20, i64* %r19
  store i64 4, i64* %_t7.addr
  %r21 = load i8*, i8** %__tmp1.addr
  %r22 = getelementptr i8, i8* %r21, i64 16
  %r23 = bitcast i8* %r22 to i64*
  %r24 = load i64, i64* %_t7.addr
  store i64 %r24, i64* %r23
  %r25 = load i8*, i8** %__tmp1.addr
  store i8* %r25, i8** %b.addr
  store i64 2, i64* %_t8.addr
  %r26 = load i64, i64* %_t8.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.1, i64 0, i64 0), i64 %r26)
  store i64 7, i64* %c.addr
  %r27 = load i64, i64* %c.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.2, i64 0, i64 0), i64 %r27)
  store i64 5, i64* %_t11.addr
  %r28 = load i64, i64* %_t11.addr
  %r29 = mul i64 %r28, 2
  store i64 %r29, i64* %_i0.addr
  %r30 = load i64, i64* %_i0.addr
  store i64 %r30, i64* %_t12.addr
  %r31 = load i64, i64* %_t12.addr
  store i64 %r31, i64* %d.addr
  %r32 = load i64, i64* %d.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.3, i64 0, i64 0), i64 %r32)
  ret i32 0
}

