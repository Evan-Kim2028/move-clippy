module loops::good;

public fun guarded(cond: bool) {
    while (cond) {
        break;
    };
}
