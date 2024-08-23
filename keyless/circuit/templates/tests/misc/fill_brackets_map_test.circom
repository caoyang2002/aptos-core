pragma circom 2.1.3;

include "helpers/misc.circom";

template fill_brackets_map_test() {
    var len = 13;
    signal input in[len];
    signal input brackets[len];
    component brackets_map = FillBracketsMap(len);
    brackets_map.arr <== in;
    for (var i = 0; i < len; i++) {
        brackets[i] === brackets_map.out[i];
    }
}

component main = fill_brackets_map_test();
