# Setting up the development environment on Debian 12

This document describes the setup of a development environment for the Prima+ÖV project on a Debian system from scratch.

After completing the necessary steps, you should be set up for coding and testing.

## Install prerequisites

The following steps need to be done in a root shell.

```
apt update
apt install ca-certificates curl gnupg apt-transport-https gpg git postgresql-client
```

## Install docker

The following steps need to be done in a root shell.

### Download GPG key and add the docker package repository to apt sources

```
curl -fsSL https://download.docker.com/linux/debian/gpg | gpg --dearmor -o /usr/share/keyrings/docker.gpg

echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker.gpg] https://download.docker.com/linux/debian bookworm stable" |tee /etc/apt/sources.list.d/docker.list > /dev/null

apt update
```

### Install Docker packages

```
apt install docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin docker-compose
```

### Add your user to the docker group

Check, if the docker group is available

```
cat /etc/group | grep docker
```

If not, create it

```
groupadd docker
```

Add your non-privileged user account to the docker group

```
usermod -aG docker <user>
```

Switch to a non-root shell.
Log your user in again, now with the new group

```
newgrp docker
```

### Start docker containers

From the project root, issue the following command:

```
docker compose up -d
```

This starts the docker containers defined in the docker-compose.yml file.

The containers are configured to automatically restart.
Therefore they should be up and running, even after a system reboot.

In order to stop them permanently, issue the command:

```
docker compose down
```

## Install node.js and npm

Download Node.js
https://nodejs.org/en/download/prebuilt-binaries/current

Extract the downloaded archive to a location of your choice.

For example: ~/tools/

### Add the node binaries to PATH

Open ~/.bashrc or the configuration file of your preferred shell and add the following line

```
export PATH=<path_where_you_extracted>/node-v22.7.0-linux-x64/bin:$PATH
```

Close your terminal session and reopen it.

### Test

Running

```
npm --version
```

should yield a version number > 10.8.2

## Prepare your local git repository

Project URL on GitHub: https://github.com/motis-project/prima

Get a GitHub account, if you don't already have one and get your fork the project.
Clone your fork to your local machine, at a location of your choice.

```
git clone git@github.com:<YOUR_FORK>.git
```

Add the prima repository as an additional remote.

```
cd prima
git remote add prima git@github.com:motis-project/prima.git
git fetch prima
```

## Install node modules

From the project root, issue the following command:

```
npm install -f
```

## Set up data base and populate it with test data

From the project root, issue the following command:

```
./test_data/load.sh default
```

## Start the dev server

```
npm run dev
```

The dev server should now serve the Prima-ÖV web page on localhost.
Press Ctrl and click the URL that should be shown in the terminal, in order to open the web application your standard browser.

## Running tests

Install Playwright Dependencies
```
npx playwright install --with-deps
```

Run integration tests
```
npm run test:integration
```

Run unit tests in watch mode
```
npm run test:unit
```

Run all tests
```
npm run test
```
