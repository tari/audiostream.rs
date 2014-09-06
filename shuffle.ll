; Interleave (8 * (n / 8)) i16 values from _x and _y to _out.
define void @interleave_i16x2_8(i16 *%_x, i16 *%_y, i16* %_out, i64 %_n) {

begin:
    %x_init = bitcast i16* %_x to <8 x i16>*
    %y_init = bitcast i16* %_y to <8 x i16>*
    %out_init = bitcast i16* %_out to <16 x i16>*
    %n_init = udiv i64 %_n, 8

    %nop = icmp eq i64 0, %n_init
    br i1 %nop, label %done, label %interleave

interleave:
    %n = phi i64 [%n_init, %begin], [%n_next, %interleave]
    %x = phi <8 x i16>* [%x_init, %begin], [%x_next, %interleave]
    %y = phi <8 x i16>* [%y_init, %begin], [%y_next, %interleave]
    %out = phi <16 x i16>* [%out_init, %begin], [%out_next, %interleave]

    %left = load <8 x i16>* %x
    %right = load <8 x i16>* %y
    %z = shufflevector <8 x i16> %left, <8 x i16> %right,
                       <16 x i32> <i32 0, i32 8, i32 1, i32 9,
                                   i32 2, i32 10, i32 3, i32 11,
                                   i32 4, i32 12, i32 5, i32 13,
                                   i32 6, i32 14, i32 7, i32 15>
    store <16 x i16> %z, <16 x i16>* %out

    %0 = ptrtoint <8 x i16>* %x to i64
    %1 = add i64 16, %0
    %x_next = inttoptr i64 %1 to <8 x i16>*
    %2 = ptrtoint <8 x i16>* %y to i64
    %3 = add i64 16, %2
    %y_next = inttoptr i64 %3 to <8 x i16>*
    %4 = ptrtoint <16 x i16>* %out to i64
    %5 = add i64 32, %4
    %out_next = inttoptr i64 %5 to <16 x i16>*
    %n_next = sub i64 1, %n

    %repeat = icmp ne i64 0, %n_next
    br i1 %repeat, label %interleave, label %done

done:
    ret void
}

