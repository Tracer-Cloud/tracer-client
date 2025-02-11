SHELL := bash
SPACK_VERSION := 0.23.0

.PHONY: download_spack
download_spack:
	@if [ ! -f spack-$(SPACK_VERSION).tar.gz ]; \
	then \
		echo "Downloading Spack $(SPACK_VERSION)"; \
		curl -LOJ https://github.com/spack/spack/releases/download/v$(SPACK_VERSION)/spack-$(SPACK_VERSION).tar.gz; \
	fi; \
	if [ ! -d spack ]; \
	then \
		rm -r spack-$(SPACK_VERSION) || true; \
		tar -xzf spack-$(SPACK_VERSION).tar.gz; \
		mv spack-$(SPACK_VERSION) spack; \
	fi; \
	. spack/share/spack/setup-env.sh; \
	spack bootstrap now

.PHONY: setup_environment
setup_environment: download_spack
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	spack install

.PHONY: test_sarek
test_sarek:
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	nextflow run pipelines/nf-core/sarek/main.nf \
		-params-file nextflow-config/sarek-params.json \
		-profile docker,arm,test

.PHONY: test_sarek_aws_batch
test_sarek_aws_batch:
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	nextflow -c nextflow-config/batch.config run pipelines/nf-core/sarek/main.nf \
		-params-file nextflow-config/sarek-params.json \
		-profile test

.PHONY: test_full_sarek_aws_batch
test_full_sarek_aws_batch:
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	nextflow -c nextflow-config/batch.config run pipelines/nf-core/sarek/main.nf \
		-params-file nextflow-config/sarek-params.json \
		-profile test_full

.PHONY: test_rnaseq
test_rnaseq:
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	nextflow run -c nextflow-config/rnaseq.config pipelines/nf-core/rnaseq/main.nf \
		-params-file nextflow-config/rnaseq-params.json \
		-profile docker,test

.PHONY: test_rnaseq_aws_batch
test_rnaseq_aws_batch:
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	nextflow -c nextflow-config/batch.config run pipelines/nf-core/rnaseq/main.nf \
		-params-file nextflow-config/rnaseq-params.json \
		-profile test

.PHONY: test_full_rnaseq_aws_batch
test_full_rnaseq_aws_batch:
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	nextflow -c nextflow-config/batch.config run pipelines/nf-core/rnaseq/main.nf \
		-params-file nextflow-config/rnaseq-params.json \
		-profile test_full

.PHONY: test_proteinfold
test_proteinfold:
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	nextflow run pipelines/nf-core/proteinfold/main.nf \
		-params-file nextflow-config/proteinfold-params.json \
		-profile docker,arm,test

.PHONY: test_proteinfold_aws_batch
test_proteinfold_aws_batch:
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	nextflow -c nextflow-config/batch.config -c nextflow-config/proteinfold.config run pipelines/nf-core/proteinfold/main.nf \
		-params-file nextflow-config/proteinfold-params.json \
		-profile test \
		--use_gpu

.PHONY: test_full_proteinfold_aws_batch
test_full_proteinfold_aws_batch:
	@. spack/share/spack/setup-env.sh; \
	spack env activate -d .; \
	nextflow -c nextflow-config/batch.config -c nextflow-config/proteinfold.config run pipelines/nf-core/proteinfold/main.nf \
		-params-file nextflow-config/proteinfold-params.json \
		-profile test_full \
		--use_gpu