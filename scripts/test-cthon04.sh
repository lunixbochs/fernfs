#!/bin/bash -eu

echo "========================================"
echo "== cthon04 test suite"
echo "========================================"
sudo umount /mnt/cthon04 || true
sudo mkdir -p /mnt/cthon04
cd ~/cthon04

# echo "----------------------------------------"
# echo "-- kernel tests"
# echo "----------------------------------------"
# ./server -a 127.0.0.1 -m /mnt/cthon04 -p /opt/nfs-kernel

echo "----------------------------------------"
echo "-- mamont tests"
echo "----------------------------------------"
./server -a 127.0.0.1 -o port=11111,mountport=11111,vers=3,proto=tcp -m /mnt/cthon04 -p /
