#!/bin/bash -e

PWD=`pwd`
DOWNLOAD_DIR=${PWD}/download
SPEC_DIR=${PWD}/spec
TMP_SPEC_DIR=${SPEC_DIR}_tmp

mkdir -p ${DOWNLOAD_DIR}
mkdir -p ${SPEC_DIR}
mkdir -p ${TMP_SPEC_DIR}

echo "DOWNLOAD_DIR=${DOWNLOAD_DIR}"
echo "SPEC_DIR=${SPEC_DIR}"

function stage_spec {
	SPEC_ID=$1
	PDF_PATH=$2
	cp ${PDF_PATH} ${TMP_SPEC_DIR}/${SPEC_ID}.pdf
}

function download_spec_pdf {
	PDF_PATH=$1
	SPEC_URL=$2
	if [[ -f "${PDF_PATH}" ]]; then
		echo "${PDF_PATH} exists. Skip downloading..."
	else
		wget --no-check-certificate --user-agent="Mozilla" -O ${PDF_PATH} ${SPEC_URL}
	fi
}

function def_spec_pdf {
	SPEC_ID=$1
	SPEC_URL=$2
	PDF_PATH="${DOWNLOAD_DIR}/${SPEC_ID}.pdf"
	download_spec_pdf ${PDF_PATH} ${SPEC_URL}
	stage_spec ${SPEC_ID} ${PDF_PATH}
}

function def_spec_zip {
	SPEC_ID=$1
	SPEC_ZIP_URL=$2
	SPEC_PDF_RELPATH=$3
	ZIP_PATH="${DOWNLOAD_DIR}/${SPEC_ID}.zip"
	if [[ -f "${ZIP_PATH}" ]]; then
		echo "${ZIP_PATH} exists. Skip downloading..."
	else
		wget --user-agent="Mozilla" -O ${ZIP_PATH} ${SPEC_ZIP_URL}
		unzip -o -d ${DOWNLOAD_DIR} ${ZIP_PATH}
	fi
	stage_spec ${SPEC_ID} ${DOWNLOAD_DIR}/${SPEC_PDF_RELPATH}
}

function deploy_spec {
	sha1sum ${TMP_SPEC_DIR}/*.pdf | \
		sed "s#${TMP_SPEC_DIR}/##g" | \
		tee ${TMP_SPEC_DIR}/index.txt
	if diff -u ${SPEC_DIR}/index.txt ${TMP_SPEC_DIR}/index.txt ; then
		echo "Files up to date"
	else
		read -p "Diff found. Do you want to update? [Enter to proceed, or Ctrl-C to cancel]"
	fi
	rm -rf ${SPEC_DIR} ; true
	mkdir -p ${SPEC_DIR}
	cp -rv ${TMP_SPEC_DIR}/* ${SPEC_DIR}
}

. download_entries.generated.sh
deploy_spec
