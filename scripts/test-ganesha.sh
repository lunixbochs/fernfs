#!/bin/bash -eu

echo "========================================"
echo "== nfs-ganesha test suite"
echo "========================================"
cd ~/nfs-ganesha/src/scripts/test_through_mountpoint
rm -rf /mnt/nfs-kernel/nfs-ganesha /mnt/nfs-mamont/nfs-ganesha
mkdir /mnt/nfs-kernel/nfs-ganesha /mnt/nfs-mamont/nfs-ganesha
for script in *.sh; do
    echo "----------------------------------------"
    echo "[+] $script (kernel)"
    bash "$script" /mnt/nfs-kernel/nfs-ganesha
    echo "[+] $script (mamont)"
    bash "$script" /mnt/nfs-mamont/nfs-ganesha
    echo
done
rm -rf /mnt/nfs-kernel/nfs-ganesha /mnt/nfs-mamont/nfs-ganesha
