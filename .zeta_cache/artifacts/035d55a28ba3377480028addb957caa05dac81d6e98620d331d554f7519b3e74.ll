; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.fmt.0 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"

define i32 @main() {
entry:
  %__tmp0.addr = alloca i8*
  %_t0.addr = alloca i8*
  %_t1.addr = alloca i64
  %_t2.addr = alloca i64
  %_t3.addr = alloca i64
  %a.addr = alloca i8*
  %x.addr = alloca i64
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
  store i64 42, i64* %_t2.addr
  %r6 = load i8*, i8** %__tmp0.addr
  %r7 = getelementptr i8, i8* %r6, i64 8
  %r8 = bitcast i8* %r7 to i64*
  %r9 = load i64, i64* %_t2.addr
  store i64 %r9, i64* %r8
  %r10 = load i8*, i8** %__tmp0.addr
  store i8* %r10, i8** %a.addr
  %r11 = load i64, i64* %a.addr
  %r12 = call i64 @"Option::unwrap__i64"(i64 %r11)
  store i64 %r12, i64* %_t3.addr
  %r13 = load i64, i64* %_t3.addr
  store i64 %r13, i64* %x.addr
  %r14 = load i64, i64* %x.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.0, i64 0, i64 0), i64 %r14)
  ret i32 0
}

define i64 @"Option::unwrap__i64"(i64 %self) {
entry:
  %__tmp1.addr = alloca i64
  %_t0.addr = alloca i64
  %_t1.addr = alloca i1
  %_t2.addr = alloca i64
  %_t3.addr = alloca i64
  %_t4.addr = alloca i64
  %_t5.addr = alloca i1
  %_t6.addr = alloca i64
  %self.addr = alloca i64
  %v.addr = alloca i64
  store i64 %self, i64* %self.addr
  %r15 = load i64, i64* %self.addr
  store i64 %r15, i64* %__tmp1.addr
  %r16 = load i8*, i8** %__tmp1.addr
  %r17 = getelementptr i8, i8* %r16, i64 0
  %r18 = bitcast i8* %r17 to i64*
  %r19 = load i64, i64* %r18
  store i64 %r19, i64* %_t0.addr
  %r20 = load i64, i64* %_t0.addr
  %r21 = icmp eq i64 %r20, 1
  store i1 %r21, i1* %_t1.addr
  %r22 = load i1, i1* %_t1.addr
  br i1 %r22, label %b1, label %b3
b1:
  %r23 = load i8*, i8** %__tmp1.addr
  %r24 = getelementptr i8, i8* %r23, i64 8
  %r25 = bitcast i8* %r24 to i64*
  %r26 = load i64, i64* %r25
  store i64 %r26, i64* %_t3.addr
  %r27 = load i64, i64* %_t3.addr
  store i64 %r27, i64* %v.addr
  %r28 = load i64, i64* %v.addr
  store i64 %r28, i64* %_t2.addr
  br label %b2
b2:
  %r29 = load i64, i64* %_t2.addr
  ret i64 %r29
b3:
  %r30 = load i8*, i8** %__tmp1.addr
  %r31 = getelementptr i8, i8* %r30, i64 0
  %r32 = bitcast i8* %r31 to i64*
  %r33 = load i64, i64* %r32
  store i64 %r33, i64* %_t4.addr
  %r34 = load i64, i64* %_t4.addr
  %r35 = icmp eq i64 %r34, 0
  store i1 %r35, i1* %_t5.addr
  %r36 = load i1, i1* %_t5.addr
  br i1 %r36, label %b4, label %b5
b4:
  br label %b6
b5:
  %r37 = load i64, i64* %_t6.addr
  store i64 %r37, i64* %_t2.addr
  br label %b2
b6:
  br label %b6
}

