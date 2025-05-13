#!/bin/bash

# FASTQC - QC layer 1
cd ./workflows/chipseq

fastqc s1_1.fq s1_2.fq -o .

# Creating a human genome index using STAR
STAR --runThreadN 4 --runMode genomeGenerate --genomeDir ./human --genomeSAindexNbases 10 --genomeFastaFiles human.fa --sjdbGTFfile hg19.refGene.gtf --sjdbOverhang 99

# Align RNA-Seq reads to the genome using STAR
STAR --runThreadN 4 --genomeDir ./human --readFilesIn s1_1.fq s1_2.fq --outFileNamePrefix P1s1 --outSAMtype BAM SortedByCoordinate

# Sort and Index the output bam file
samtools sort *.bam -@ 4 -o P1s1.sorted.bam
samtools index P1s1.sorted.bam

# Call Peaks for BAM file
macs3 callpeak -t P1s1.sorted.bam -f BAMPE -p 0.05 --outdir ./Peaks
