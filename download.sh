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
		wget --user-agent="Mozilla" -O ${PDF_PATH} ${SPEC_URL}
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

def_spec_pdf sdm_vol1 https://cdrdv2.intel.com/v1/dl/getContent/671436
def_spec_pdf sdm_vol2 https://cdrdv2.intel.com/v1/dl/getContent/671110
def_spec_pdf sdm_vol3 https://cdrdv2.intel.com/v1/dl/getContent/671447
def_spec_pdf sdm_vol4 https://cdrdv2.intel.com/v1/dl/getContent/671098
def_spec_pdf uefi_2_9 https://uefi.org/sites/default/files/resources/UEFI_Spec_2_9_2021_03_18.pdf
def_spec_pdf acpi_6_4 https://uefi.org/sites/default/files/resources/ACPI_Spec_6_4_Jan22.pdf
def_spec_pdf xhci_1_2 https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/extensible-host-controler-interface-usb-xhci.pdf
def_spec_pdf armv8a_pg_1_0 https://documentation-service.arm.com/static/5fbd26f271eff94ef49c7020
def_spec_zip usb_2_0 https://www.usb.org/sites/default/files/usb_20_20190524.zip usb_20_20190524/usb_20.pdf
def_spec_zip cdc_1_2 https://www.usb.org/sites/default/files/CDC1.2_WMC1.1_012011.zip CDC1.2_WMC1.1_012011/CDC1.2_WMC1.1/usbcdc12/CDC120-20101103-track.pdf
def_spec_zip ecm_1_2 https://www.usb.org/sites/default/files/CDC1.2_WMC1.1_012011.zip CDC1.2_WMC1.1_012011/CDC1.2_WMC1.1/usbcdc12/CDC120-20101103-track.pdf
deploy_spec
