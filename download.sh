#!/bin/bash -x -e

mkdir -p download
mkdir -p spec
wget -N -P download/ https://www.usb.org/sites/default/files/CDC1.2_WMC1.1_012011.zip
unzip -o -d spec/ download/CDC1.2_WMC1.1_012011.zip
wget -N -P download/ https://www.usb.org/sites/default/files/usb_20_20190524.zip
unzip -o -d spec/ download/usb_20_20190524.zip
wget --user-agent="Mozilla" -N -P spec/ https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/extensible-host-controler-interface-usb-xhci.pdf
