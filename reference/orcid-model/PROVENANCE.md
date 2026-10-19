# ORCID message schema v3.0 (reference copy)

Source: https://github.com/ORCID/orcid-model at commit `9592e2d3bde21a1edf703f26f8bc304448f886fb`
Paths:  src/main/resources/record_3.0/ (README.md, *.xsd, samples/)
        src/main/resources/common_3.0/ (shared types imported by the record XSDs)
        src/main/resources/group-id-3.0/ (peer-review group ids: README.md, the XSD, samples/)
License: ORCID MIT-style license, see LICENSE in this directory.

Unmodified. This is the specification the crate's model, readers, writers and
validation are written against, and the test suite reads these files (see
../../NOTICE). The three directories are the subset of ORCID's v3.0 resources
the tests use (the record XSDs import ../common_3.0/common-3.0.xsd;
group-id-3.0.xsd imports ../common_2.1/common-2.1.xsd, which is not copied, so
the group-id XSD is read by the tests but not used to validate). Refresh by
re-running the copy at a newer commit.
