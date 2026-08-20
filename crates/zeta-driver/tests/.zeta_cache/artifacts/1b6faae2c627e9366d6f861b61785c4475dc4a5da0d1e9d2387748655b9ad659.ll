; ModuleID = 'zeta'
declare i32 @printf(i8*, ...)
declare i8* @malloc(i64)
@.str.0 = private unnamed_addr constant [15 x i8] c"sum half-open:\00"
@.fmt.1 = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.fmt.2 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.str.3 = private unnamed_addr constant [12 x i8] c"sum closed:\00"
@.fmt.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.fmt.5 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.str.6 = private unnamed_addr constant [16 x i8] c"sum open-lower:\00"
@.fmt.7 = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.fmt.8 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.str.9 = private unnamed_addr constant [15 x i8] c"evens 0+2+4 = \00"
@.fmt.10 = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.fmt.11 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.str.12 = private unnamed_addr constant [20 x i8] c"nested product sum:\00"
@.fmt.13 = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.fmt.14 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.str.15 = private unnamed_addr constant [22 x i8] c"sum to n (0+..+6=21):\00"
@.fmt.16 = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.fmt.17 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"

define i32 @main() {
entry:
  %__for_hi_1.addr = alloca i64
  %__for_hi_11.addr = alloca i64
  %__for_hi_13.addr = alloca i64
  %__for_hi_3.addr = alloca i64
  %__for_hi_5.addr = alloca i64
  %__for_hi_7.addr = alloca i64
  %__for_hi_9.addr = alloca i64
  %__for_lo_0.addr = alloca i64
  %__for_lo_10.addr = alloca i64
  %__for_lo_12.addr = alloca i64
  %__for_lo_2.addr = alloca i64
  %__for_lo_4.addr = alloca i64
  %__for_lo_6.addr = alloca i64
  %__for_lo_8.addr = alloca i64
  %_t0.addr = alloca i1
  %_t1.addr = alloca i64
  %_t13.addr = alloca i8*
  %_t16.addr = alloca i1
  %_t17.addr = alloca i64
  %_t21.addr = alloca i8*
  %_t24.addr = alloca i1
  %_t25.addr = alloca i1
  %_t27.addr = alloca i1
  %_t29.addr = alloca i64
  %_t33.addr = alloca i8*
  %_t36.addr = alloca i1
  %_t37.addr = alloca i1
  %_t38.addr = alloca i64
  %_t39.addr = alloca i64
  %_t43.addr = alloca i64
  %_t47.addr = alloca i8*
  %_t5.addr = alloca i8*
  %_t50.addr = alloca i1
  %_t51.addr = alloca i64
  %_t55.addr = alloca i8*
  %_t59.addr = alloca i64
  %_t8.addr = alloca i1
  %_t9.addr = alloca i64
  %a.addr = alloca i64
  %b.addr = alloca i64
  %evens.addr = alloca i64
  %i.addr = alloca i64
  %n.addr = alloca i64
  %product.addr = alloca i64
  %s.addr = alloca i64
  %sum.addr = alloca i64
  %sum2.addr = alloca i64
  %sum3.addr = alloca i64
  store i64 0, i64* %sum.addr
  store i64 0, i64* %__for_lo_0.addr
  store i64 10, i64* %__for_hi_1.addr
  %r0 = load i64, i64* %__for_lo_0.addr
  store i64 %r0, i64* %i.addr
  br label %b1
b1:
  %r1 = load i64, i64* %i.addr
  %r2 = load i64, i64* %__for_hi_1.addr
  %r3 = icmp slt i64 %r1, %r2
  store i1 %r3, i1* %_t0.addr
  %r4 = load i1, i1* %_t0.addr
  br i1 %r4, label %b2, label %b3
b2:
  %r5 = load i64, i64* %sum.addr
  %r6 = load i64, i64* %i.addr
  %r7 = add i64 %r5, %r6
  store i64 %r7, i64* %sum.addr
  store i64 1, i64* %_t1.addr
  %r8 = load i64, i64* %i.addr
  %r9 = load i64, i64* %_t1.addr
  %r10 = add i64 %r8, %r9
  store i64 %r10, i64* %i.addr
  br label %b1
b3:
  store i8* getelementptr inbounds ([15 x i8], [15 x i8]* @.str.0, i64 0, i64 0), i8** %_t5.addr
  %r11 = load i8*, i8** %_t5.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.fmt.1, i64 0, i64 0), i8* %r11)
  %r12 = load i64, i64* %sum.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.2, i64 0, i64 0), i64 %r12)
  store i64 0, i64* %sum2.addr
  store i64 0, i64* %__for_lo_2.addr
  store i64 5, i64* %__for_hi_3.addr
  %r13 = load i64, i64* %__for_lo_2.addr
  store i64 %r13, i64* %i.addr
  br label %b4
b4:
  %r14 = load i64, i64* %i.addr
  %r15 = load i64, i64* %__for_hi_3.addr
  %r16 = icmp sle i64 %r14, %r15
  store i1 %r16, i1* %_t8.addr
  %r17 = load i1, i1* %_t8.addr
  br i1 %r17, label %b5, label %b6
b5:
  %r18 = load i64, i64* %sum2.addr
  %r19 = load i64, i64* %i.addr
  %r20 = add i64 %r18, %r19
  store i64 %r20, i64* %sum2.addr
  store i64 1, i64* %_t9.addr
  %r21 = load i64, i64* %i.addr
  %r22 = load i64, i64* %_t9.addr
  %r23 = add i64 %r21, %r22
  store i64 %r23, i64* %i.addr
  br label %b4
b6:
  store i8* getelementptr inbounds ([12 x i8], [12 x i8]* @.str.3, i64 0, i64 0), i8** %_t13.addr
  %r24 = load i8*, i8** %_t13.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.fmt.4, i64 0, i64 0), i8* %r24)
  %r25 = load i64, i64* %sum2.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.5, i64 0, i64 0), i64 %r25)
  store i64 0, i64* %sum3.addr
  store i64 0, i64* %__for_lo_4.addr
  store i64 5, i64* %__for_hi_5.addr
  %r26 = load i64, i64* %__for_lo_4.addr
  %r27 = add i64 %r26, 1
  store i64 %r27, i64* %i.addr
  br label %b7
b7:
  %r28 = load i64, i64* %i.addr
  %r29 = load i64, i64* %__for_hi_5.addr
  %r30 = icmp sle i64 %r28, %r29
  store i1 %r30, i1* %_t16.addr
  %r31 = load i1, i1* %_t16.addr
  br i1 %r31, label %b8, label %b9
b8:
  %r32 = load i64, i64* %sum3.addr
  %r33 = load i64, i64* %i.addr
  %r34 = add i64 %r32, %r33
  store i64 %r34, i64* %sum3.addr
  store i64 1, i64* %_t17.addr
  %r35 = load i64, i64* %i.addr
  %r36 = load i64, i64* %_t17.addr
  %r37 = add i64 %r35, %r36
  store i64 %r37, i64* %i.addr
  br label %b7
b9:
  store i8* getelementptr inbounds ([16 x i8], [16 x i8]* @.str.6, i64 0, i64 0), i8** %_t21.addr
  %r38 = load i8*, i8** %_t21.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.fmt.7, i64 0, i64 0), i8* %r38)
  %r39 = load i64, i64* %sum3.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.8, i64 0, i64 0), i64 %r39)
  store i64 0, i64* %evens.addr
  store i64 0, i64* %__for_lo_6.addr
  store i64 100, i64* %__for_hi_7.addr
  %r40 = load i64, i64* %__for_lo_6.addr
  store i64 %r40, i64* %i.addr
  br label %b10
b10:
  %r41 = load i64, i64* %i.addr
  %r42 = load i64, i64* %__for_hi_7.addr
  %r43 = icmp slt i64 %r41, %r42
  store i1 %r43, i1* %_t24.addr
  %r44 = load i1, i1* %_t24.addr
  br i1 %r44, label %b11, label %b12
b11:
  %r45 = load i64, i64* %i.addr
  %r46 = icmp sge i64 %r45, 6
  store i1 %r46, i1* %_t25.addr
  %r47 = load i1, i1* %_t25.addr
  br i1 %r47, label %b13, label %b14
b12:
  store i8* getelementptr inbounds ([15 x i8], [15 x i8]* @.str.9, i64 0, i64 0), i8** %_t33.addr
  %r48 = load i8*, i8** %_t33.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.fmt.10, i64 0, i64 0), i8* %r48)
  %r49 = load i64, i64* %evens.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.11, i64 0, i64 0), i64 %r49)
  store i64 0, i64* %product.addr
  store i64 1, i64* %__for_lo_8.addr
  store i64 4, i64* %__for_hi_9.addr
  %r50 = load i64, i64* %__for_lo_8.addr
  store i64 %r50, i64* %a.addr
  br label %b17
b13:
  br label %b12
b14:
  %r51 = load i64, i64* %i.addr
  %r52 = srem i64 %r51, 2
  store i64 %r52, i64* %_t59.addr
  %r53 = load i64, i64* %_t59.addr
  %r54 = icmp ne i64 %r53, 0
  store i1 %r54, i1* %_t27.addr
  %r55 = load i1, i1* %_t27.addr
  br i1 %r55, label %b15, label %b16
b15:
  br label %b10
b16:
  %r56 = load i64, i64* %evens.addr
  %r57 = load i64, i64* %i.addr
  %r58 = add i64 %r56, %r57
  store i64 %r58, i64* %evens.addr
  store i64 1, i64* %_t29.addr
  %r59 = load i64, i64* %i.addr
  %r60 = load i64, i64* %_t29.addr
  %r61 = add i64 %r59, %r60
  store i64 %r61, i64* %i.addr
  br label %b10
b17:
  %r62 = load i64, i64* %a.addr
  %r63 = load i64, i64* %__for_hi_9.addr
  %r64 = icmp slt i64 %r62, %r63
  store i1 %r64, i1* %_t36.addr
  %r65 = load i1, i1* %_t36.addr
  br i1 %r65, label %b18, label %b19
b18:
  store i64 1, i64* %__for_lo_10.addr
  store i64 4, i64* %__for_hi_11.addr
  %r66 = load i64, i64* %__for_lo_10.addr
  store i64 %r66, i64* %b.addr
  br label %b20
b19:
  store i8* getelementptr inbounds ([20 x i8], [20 x i8]* @.str.12, i64 0, i64 0), i8** %_t47.addr
  %r67 = load i8*, i8** %_t47.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.fmt.13, i64 0, i64 0), i8* %r67)
  %r68 = load i64, i64* %product.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.14, i64 0, i64 0), i64 %r68)
  store i64 7, i64* %n.addr
  store i64 0, i64* %s.addr
  store i64 0, i64* %__for_lo_12.addr
  %r69 = load i64, i64* %n.addr
  store i64 %r69, i64* %__for_hi_13.addr
  %r70 = load i64, i64* %__for_lo_12.addr
  store i64 %r70, i64* %i.addr
  br label %b23
b20:
  %r71 = load i64, i64* %b.addr
  %r72 = load i64, i64* %__for_hi_11.addr
  %r73 = icmp slt i64 %r71, %r72
  store i1 %r73, i1* %_t37.addr
  %r74 = load i1, i1* %_t37.addr
  br i1 %r74, label %b21, label %b22
b21:
  %r75 = load i64, i64* %a.addr
  %r76 = load i64, i64* %b.addr
  %r77 = mul i64 %r75, %r76
  store i64 %r77, i64* %_t38.addr
  %r78 = load i64, i64* %product.addr
  %r79 = load i64, i64* %_t38.addr
  %r80 = add i64 %r78, %r79
  store i64 %r80, i64* %product.addr
  store i64 1, i64* %_t39.addr
  %r81 = load i64, i64* %b.addr
  %r82 = load i64, i64* %_t39.addr
  %r83 = add i64 %r81, %r82
  store i64 %r83, i64* %b.addr
  br label %b20
b22:
  store i64 1, i64* %_t43.addr
  %r84 = load i64, i64* %a.addr
  %r85 = load i64, i64* %_t43.addr
  %r86 = add i64 %r84, %r85
  store i64 %r86, i64* %a.addr
  br label %b17
b23:
  %r87 = load i64, i64* %i.addr
  %r88 = load i64, i64* %__for_hi_13.addr
  %r89 = icmp slt i64 %r87, %r88
  store i1 %r89, i1* %_t50.addr
  %r90 = load i1, i1* %_t50.addr
  br i1 %r90, label %b24, label %b25
b24:
  %r91 = load i64, i64* %s.addr
  %r92 = load i64, i64* %i.addr
  %r93 = add i64 %r91, %r92
  store i64 %r93, i64* %s.addr
  store i64 1, i64* %_t51.addr
  %r94 = load i64, i64* %i.addr
  %r95 = load i64, i64* %_t51.addr
  %r96 = add i64 %r94, %r95
  store i64 %r96, i64* %i.addr
  br label %b23
b25:
  store i8* getelementptr inbounds ([22 x i8], [22 x i8]* @.str.15, i64 0, i64 0), i8** %_t55.addr
  %r97 = load i8*, i8** %_t55.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.fmt.16, i64 0, i64 0), i8* %r97)
  %r98 = load i64, i64* %s.addr
  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.fmt.17, i64 0, i64 0), i64 %r98)
  ret i32 0
}

