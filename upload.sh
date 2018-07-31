#!/bin/bash
ssh vps "mv genericbot-rs/genericbot-rs genericbot-rs/genericbot-rs.old"
scp target/release/genericbot-rs vps:genericbot-rs
ssh vps "sudo systemctl restart bot"
