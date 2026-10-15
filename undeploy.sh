#!/usr/bin/env bash
# Remove the Deck Calendar widget, installed with ./deploy.sh, from a Braiins Deck.
#
# Usage: ./undeploy.sh <deck-ip>
set -euo pipefail

DECK_IP="${1:?usage: ./undeploy.sh <deck-ip>}"

# The package manager shells out to nix-store, which a plain SSH session doesn't have on its PATH.
ssh -o BatchMode=yes -o ConnectTimeout=10 "root@$DECK_IP" \
	"PATH=/run/current-profile/bin:\$PATH /nix/var/nix/gcroots/profiles/bmc/current/bin/bmc-nix-cli remove-packages --name widget-deck-calendar"

echo "Removed the Deck Calendar widget from the Deck at $DECK_IP."
