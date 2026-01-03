.PHONY: lima-env lima-cargo test-ganesha test-cthon04

lima-env:
	limactl delete -f nfs-mamont
	limactl start --progress --name nfs-mamont --set ".param.AppDir=\"$(shell pwd)\"" -y lima.yaml

lima-cargo:
	limactl shell --workdir /mnt/app nfs-mamont -- bash -c 'cargo build --examples && sudo systemctl restart nfs-mamont && sudo umount /mnt/nfs-mamont && sudo mount /mnt/nfs-mamont'

test-ganesha: lima-cargo
	limactl shell nfs-mamont -- /mnt/app/scripts/test-ganesha.sh

test-cthon04: lima-cargo
	limactl shell nfs-mamont -- /mnt/app/scripts/test-cthon04.sh

test-pjdfstest: lima-cargo
	limactl shell nfs-mamont -- /mnt/app/scripts/test-pjdfstest.sh
