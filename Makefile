.PHONY: lima-env lima-cargo test-ganesha test-cthon04

lima-env:
	limactl delete -f fernfs
	limactl start --progress --name fernfs --set ".param.AppDir=\"$(shell pwd)\"" -y lima.yaml

lima-cargo:
	limactl shell --workdir /mnt/app fernfs -- bash -c 'cargo build --examples && sudo systemctl restart fernfs && sudo umount /mnt/fernfs && sudo mount /mnt/fernfs'

test-ganesha: lima-cargo
	limactl shell fernfs -- /mnt/app/scripts/test-ganesha.sh

test-cthon04: lima-cargo
	limactl shell fernfs -- /mnt/app/scripts/test-cthon04.sh

test-pjdfstest: lima-cargo
	limactl shell fernfs -- /mnt/app/scripts/test-pjdfstest.sh
