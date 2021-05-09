#!/bin/bash

trap 'exit' INT
# handler()
# {
#     kill -s SIGINT $PID
# }

# first argument is the tty device to use


stty -F $1 115200 cs8 -cstopb -parenb

while true
do
    echo Starting defmt-print
    cat $1 | defmt-print -e target/thumb*/debug/dsaclk
    #  &
    # wait
    
done

