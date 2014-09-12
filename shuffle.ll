define void @interleave_i16x4(i16 *%_x, i16 *%_y, i16* %_out, i64 %_n) {

begin:
    %a_init = bitcast i16* %_x to <4 x i16>*
    %b_init = bitcast i16* %_y to <4 x i16>*
    %c_init = bitcast i16* %_y to <4 x i16>*
    %d_init = bitcast i16* %_y to <4 x i16>*
    %out_init = bitcast i16* %_out to <16 x i16>*
    %n_init = udiv i64 %_n, 4

    %nop = icmp eq i64 0, %n_init
    br i1 %nop, label %done, label %interleave

interleave:
    %n = phi i64 [%n_init, %begin], [%n_next, %interleave]
    %a = phi <4 x i16>* [%a_init, %begin], [%a_next, %interleave]
    %b = phi <4 x i16>* [%b_init, %begin], [%b_next, %interleave]
    %c = phi <4 x i16>* [%c_init, %begin], [%c_next, %interleave]
    %d = phi <4 x i16>* [%d_init, %begin], [%d_next, %interleave]
    %out = phi <16 x i16>* [%out_init, %begin], [%out_next, %interleave]

    %left = load <4 x i16>* %a
    %right = load <4 x i16>* %b
    %leftrear = load <4 x i16>* %c
    %rightrear = load <4 x i16>* %d

    %z = shufflevector <4 x i16> %left, <4 x i16> %right,
                       <16 x i32> <i32 0, i32 4, i32 undef, i32 undef,
                                   i32 1, i32 5, i32 undef, i32 undef,
                                   i32 2, i32 6, i32 undef, i32 undef,
                                   i32 3, i32 7, i32 undef, i32 undef>
    %rear = shufflevector <4 x i16> %leftrear, <4 x i16> %rightrear,
                          <16 x i32> <i32 0, i32 4, i32 undef, i32 undef,
                                      i32 1, i32 5, i32 undef, i32 undef,
                                      i32 2, i32 6, i32 undef, i32 undef,
                                      i32 3, i32 7, i32 undef, i32 undef>
    %z2 = shufflevector <16 x i16> %z, <16 x i16> %rear,
                        <16 x i32> <i32 0, i32 1, i32 16, i32 20,
                                    i32 4, i32 5, i32 17, i32 21,
                                    i32 8, i32 9, i32 18, i32 22,
                                    i32 12, i32 13, i32 19, i32 23>
    store <16 x i16> %z2, <16 x i16>* %out

    %0 = ptrtoint <4 x i16>* %a to i64
    %1 = add i64 8, %0
    %a_next = inttoptr i64 %1 to <4 x i16>*
    %2 = ptrtoint <4 x i16>* %b to i64
    %3 = add i64 8, %2
    %b_next = inttoptr i64 %3 to <4 x i16>*
    %4 = ptrtoint <4 x i16>* %c to i64
    %5 = add i64 8, %4
    %c_next = inttoptr i64 %5 to <4 x i16>*
    %6 = ptrtoint <4 x i16>* %d to i64
    %7 = add i64 8, %6
    %d_next = inttoptr i64 %7 to <4 x i16>*
    %8 = ptrtoint <16 x i16>* %out to i64
    %9 = add i64 32, %8
    %out_next = inttoptr i64 %9 to <16 x i16>*
    %n_next = sub i64 1, %n

    %repeat = icmp ne i64 0, %n_next
    br i1 %repeat, label %interleave, label %done

done:
    ret void
}

