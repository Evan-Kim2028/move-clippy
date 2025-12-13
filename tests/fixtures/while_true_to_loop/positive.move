module loops::bad;

public fun spin() {
    while (true) {
        break;
    };
}
