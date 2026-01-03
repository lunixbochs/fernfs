#!/bin/bash -eu

echo "========================================"
echo "== pjdfstest suite"
echo "========================================"

echo "----------------------------------------"
echo "-- kernel tests"
echo "----------------------------------------"
cd /mnt/nfs-kernel && rm -rf pjdfstest && mkdir pjdfstest && cd pjdfstest
sudo prove -rv ~/pjdfstest/tests

echo "----------------------------------------"
echo "-- mamont tests"
echo "----------------------------------------"
cd /mnt/nfs-mamont && rm -rf pjdfstest && mkdir pjdfstest && cd pjdfstest
sudo prove -rv ~/pjdfstest/tests
