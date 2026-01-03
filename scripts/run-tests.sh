#!/bin/bash -eu

echo "========================================"
echo "== nfs-ganesha test suite"
echo "========================================"
cd ~/nfs-ganesha/src/scripts/test_through_mountpoint
for script in *.sh; do
    echo "----------------------------------------"
    echo "[+] $script (kernel)"
    bash "$script" /mnt/nfs-kernel/nfs-ganesha
    echo "[+] $script (mamont)"
    bash "$script" /mnt/nfs-mamont/nfs-ganesha
    echo
done
rm -rf /mnt/nfs-kernel/nfs-ganesha
rm -rf /mnt/nfs-mamont/nfs-ganesha

echo
echo "========================================"
echo "== cthon04 test suite"
echo "========================================"
cd ~/cthon04

echo "----------------------------------------"
echo "-- kernel tests"
echo "----------------------------------------"
./server -a 127.0.0.1 -m /mnt/cthon04 -p /opt/nfs-kernel

echo "----------------------------------------"
echo "-- mamont tests"
echo "----------------------------------------"
./server -a 127.0.0.1 -o port=11111,mountport=11111,vers=3,proto=tcp -m /mnt/cthon04 -p /
