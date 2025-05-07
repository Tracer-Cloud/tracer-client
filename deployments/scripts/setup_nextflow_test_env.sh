#!/bin/bash

set -e  # Exit immediately if a command exits with a non-zero status.
set -u  # Treat unset variables as an error.

# Set non-interactive mode for apt-get
export DEBIAN_FRONTEND=noninteractive
export TZ=Etc/UTC

# Update and install core utilities
echo "Updating and installing core utilities..."
sudo apt-get update --quiet
sudo apt-get install --quiet --yes --no-install-recommends \
    gnupg \
    ca-certificates \
    apt-transport-https \
    wget \
    curl \
    sudo \
    git \
    unzip \
    graphviz \
    tree \
    software-properties-common \
    libarchive-dev

# Install Java (SapMachine) Use this instead. https://rushiinfotech.in/java-17-installation-on-ubuntu-22-04-lts/ ??
echo "Installing Java ..."
sudo apt install --quiet --yes openjdk-17-jdk

# Install Miniconda
echo "Installing Miniconda..."

# Detect system architecture
ARCH=$(uname -m)

if [[ "$ARCH" == "x86_64" ]]; then
    INSTALLER="Miniconda3-latest-Linux-x86_64.sh"
elif [[ "$ARCH" == "aarch64" ]]; then
    INSTALLER="Miniconda3-latest-Linux-aarch64.sh"
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

mkdir -p ~/miniconda3
wget "https://repo.anaconda.com/miniconda/$INSTALLER" -O ~/miniconda3/miniconda.sh

sudo bash ~/miniconda3/miniconda.sh -b -u -p /opt/conda

rm ~/miniconda3/miniconda.sh
    
# Add Conda to PATH
export PATH="/opt/conda/bin:$PATH"
echo 'export PATH="/opt/conda/bin:$PATH"' >> ~/.bashrc

# Apply the changes to the current shell session
source ~/.bashrc
sudo chown -R $USER:$USER /opt/conda/
echo "Completed Miniconda Installation..."

source ~/.bashrc
# Configure Conda and install Nextflow + packages
echo "Configuring Conda and installing packages..."
conda config --add channels defaults
conda config --add channels bioconda
conda config --add channels conda-forge
conda config --set channel_priority strict

# optional
conda install -n base libarchive -c main --force-reinstall --solver classic

echo "Installing First sets..."
conda install --quiet --yes --name base \
    nextflow \
    nf-core \
    python \
    salmon \
    deeptools \



echo "Installing Second sets..."
conda install --quiet --yes --name base \
    boost \
    star \
    macs3 \
    multiqc \
    subread \
    kallisto \
    hisat2 \
    bwa \
    bowtie2 \
    fastqc \



echo "Installing Third sets..."
conda install --quiet --yes --name base \
    gawk \
    samtools \
    mamba \
    nf-test \
    stringtie \
    black \
    prettier \
    pre-commit \
    pytest-workflow \
    snakemake \


echo "Installing Last sets..."
conda install --quiet --yes --name base \
    airflow \
    trimmomatic \
    picard \
    gatk4 \
    snpeff \
    cnvkit


echo "For Andrew's pipelines extra"
conda install -c bioconda  fq=0.12.0

conda install --quiet --yes --name base \
    trim-galore \
    bbmap \
    qualimap \
    bedtools \
    rseqc \
    ucsc-bedclip \
    ucsc-bedgraphtobigwig \
    kraken2 \
    bracken \
    # preseq \





    


echo "Cleaning up..."
conda clean --all --force-pkgs-dirs --yes

# Install R and dependencies
echo "Installing R and dependencies..."
sudo apt-get update --quiet
sudo apt-get install --quiet --yes --no-install-recommends \
    tzdata \
    r-base \
    libxml2-dev \
    libcurl4-openssl-dev \
    libssl-dev \
    libfontconfig1-dev \
    libharfbuzz-dev \
    libfribidi-dev \
    libfreetype6-dev \
    libpng-dev \
    libtiff5-dev \
    libjpeg-dev \
    libgit2-dev \
    libglpk-dev \
    make \
    build-essential
sudo rm -rf /var/lib/apt/lists/*

# Configure writable R library path
export R_LIBS_USER=/usr/local/lib/R/site-library

# Persist the setting for future sessions
echo 'export R_LIBS_USER=/usr/local/lib/R/site-library' >> ~/.bashrc
source ~/.bashrc

# Ensure the directory exists and has correct permissions
sudo mkdir -p "$R_LIBS_USER"
sudo chmod -R 777 "$R_LIBS_USER"

# Install R packages Would need to use 4.4 and above because 
# ‘MASS’ version 7.3-64 is in the repositories but depends on R (>= 4.4.0) so this doesn't quite work

echo "Installing R packages..."
R -e "install.packages(c('BiocManager', 'ggplot2'), repos='http://cran.rstudio.com/')" || echo "R package installation (CRAN) failed, continuing..."
R -e "BiocManager::install(c('DESeq2', 'tximport', 'apeglm', 'edgeR', 'limma', 'EnhancedVolcano', 'dupRadar', 'tximeta', 'pheatmap'))" || echo "R package installation (Bioconductor) failed, continuing..."

# consider installing R alternatives:
conda install -c bioconda bioconductor-deseq2 bioconductor-edger r-locfit

# Pin Nextflow version and verify
# export NXF_EDGE=1
# export NXF_VER=24.02.0-edge
echo 'export NXF_EDGE=1' >> ~/.bashrc
echo 'export NXF_VER=25.02.1-edge' >> ~/.bashrc
source ~/.bashrc


nextflow self-update
nextflow -version


# Set Nextflow work directory
# docker version
export NXF_WORK=/nextflow_work
# Clean up
unset JAVA_TOOL_OPTIONS
echo "Setup complete."



# Some notes:
# For Bashrc make sure you set the dir properly. 
# You should be able provide a valid path to data/ dir where the script can cd into

# Clone the bashrc test repository
git clone https://github.com/TracerBio/tracer-workflow-templates.git /tmp/temp-scripts

# Create necessary directories and copy files
sudo mkdir -p /workspace/bashrc_scripts
sudo cp -R /tmp/temp-scripts/shell-tracer-autoinstrumentation/ /workspace/bashrc_scripts

sudo mkdir -p /workspace/nextflow_scripts
sudo cp -R /tmp/temp-scripts/nextflow-tracer-autoinstrumentation/ /workspace/nextflow_scripts

sudo mkdir -p /workspace/tracer-workflow-templates/data
sudo cp -R /tmp/temp-scripts/data/ /workspace/tracer-workflow-templates

sudo mkdir -p /workspace/data
sudo cp -R /tmp/temp-scripts/data/ /workspace/data

# Clean up temporary files
rm -rf /tmp/temp-scripts

# Set executable permissions
sudo chmod -R +x /workspace/bashrc_scripts
sudo chmod -R +x /workspace/nextflow_scripts

sudo chown -R $USER:$USER /workspace/

source ~/.bashrc


# Setting Up Andrew's test pipelines
echo "Cloning private test pipelines"

cd && git clone https://github.com/Tracer-Cloud/tracer-test-pipelines-bioinformatics.git --recurse-submodules

# download all dependencies and checkout to tweaks branch
cd ~/tracer-test-pipelines-bioinformatics && make && git fetch && git checkout david/ami_config_fix
