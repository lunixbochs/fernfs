.PHONY: lima-env testsuite

lima-env:
	limactl delete -f nfs-mamont
	limactl start --progress --name nfs-mamont --set ".param.AppDir=\"$(shell pwd)\"" -y lima.yaml

testsuite:
	limactl shell nfs-mamont --workdir /mnt/app -- cargo build --examples
	limactl shell nfs-mamont -- /mnt/app/scripts/run-tests.sh
