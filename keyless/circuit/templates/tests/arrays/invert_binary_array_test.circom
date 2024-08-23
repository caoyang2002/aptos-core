pragma circom 2.1.3;

include "helpers/arrays.circom";

template invert_binary_array_test(len) {
    signal input in[len];
    signal input expected_out[len];
    
    signal out[len] <== InvertBinaryArray(len)(in);
    for (var i = 0; i < len; i++) {
        out[i] === expected_out[i];
    }
}

component main = invert_binary_array_test(
   4
);
