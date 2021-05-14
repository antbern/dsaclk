#!/bin/bash

# first argument is the tty device to use

# exit on Ctrl-C
trap 'exit' INT


# continually execute defmt-print in case it gets malformed data and decides to exit
while true
do
    echo Starting defmt-print
    stty -F $1 115200 raw
    cat $1 | defmt-print -e target/thumb*/debug/dsaclk    
done

