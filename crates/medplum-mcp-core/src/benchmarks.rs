//! Token Distillation Benchmark Suite & Realistic FHIR R4 Synthetic Fixtures.
//!
//! Evaluates token and byte reductions across compact, standard, and executive tiers.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::token_diet::{distill_resource, DetailLevel};

/// Estimate token count for a JSON payload (~4 characters per token).
pub fn estimate_tokens(payload: &Value) -> usize {
    let text = serde_json::to_string(payload).unwrap_or_default();
    std::cmp::max(1, text.len().div_ceil(4))
}

fn serialize_bytes(payload: &Value) -> usize {
    serde_json::to_vec(payload).map(|b| b.len()).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Realistic Synthetic FHIR R4 Fixtures
// ---------------------------------------------------------------------------

pub static SYNTHETIC_PATIENT: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "resourceType": "Patient",
        "id": "pat-synthetic-001",
        "meta": {
            "versionId": "3",
            "lastUpdated": "2026-03-15T12:00:00.000Z",
            "source": "urn:oid:2.16.840.1.113883.3.1937.777.1",
            "profile": [
                "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient|3.1.1",
                "http://hl7.org/fhir/StructureDefinition/Patient"
            ],
            "security": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
                    "code": "NORMAL",
                    "display": "Normal"
                }
            ],
            "tag": [
                {
                    "system": "http://example.org/tags",
                    "code": "vip-registry",
                    "display": "VIP Patient"
                }
            ]
        },
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\"><div class=\"hapiHeaderText\">Jane A. <b>SMITH </b></div><table class=\"hapiPropertyTable\"><tbody><tr><td>Identifier</td><td>MRN-12345</td></tr><tr><td>Date of birth</td><td>15 January 1980</td></tr><tr><td>Gender</td><td>Female</td></tr><tr><td>Address</td><td>123 Healthcare Ave, Boston, MA 02115</td></tr></tbody></table></div>"
        },
        "extension": [
            {
                "url": "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race",
                "extension": [
                    {
                        "url": "ombCategory",
                        "valueCoding": {
                            "system": "urn:oid:2.16.840.1.113883.6.238",
                            "code": "2106-3",
                            "display": "White"
                        }
                    },
                    {"url": "text", "valueString": "White"}
                ]
            },
            {
                "url": "http://hl7.org/fhir/us/core/StructureDefinition/us-core-ethnicity",
                "extension": [
                    {
                        "url": "ombCategory",
                        "valueCoding": {
                            "system": "urn:oid:2.16.840.1.113883.6.238",
                            "code": "2186-5",
                            "display": "Not Hispanic or Latino"
                        }
                    },
                    {"url": "text", "valueString": "Not Hispanic or Latino"}
                ]
            },
            {
                "url": "http://hl7.org/fhir/us/core/StructureDefinition/us-core-birthsex",
                "valueCode": "F"
            },
            {
                "url": "http://example.org/fhir/StructureDefinition/patient-portal-registered",
                "valueBoolean": true
            }
        ],
        "identifier": [
            {
                "use": "usual",
                "type": {
                    "coding": [
                        {
                            "system": "http://terminology.hl7.org/CodeSystem/v2-0203",
                            "code": "MR",
                            "display": "Medical Record Number"
                        }
                    ],
                    "text": "Hospital MRN"
                },
                "system": "urn:oid:1.2.840.114350.1.13.0.1.7.1.1",
                "value": "MRN-12345",
                "assigner": {"display": "Mass General Brigham Clinical Health"}
            },
            {
                "use": "official",
                "type": {
                    "coding": [
                        {
                            "system": "http://terminology.hl7.org/CodeSystem/v2-0203",
                            "code": "SS",
                            "display": "Social Security Number"
                        }
                    ]
                },
                "system": "http://hl7.org/fhir/sid/us-ssn",
                "value": "000-12-3456"
            }
        ],
        "active": true,
        "name": [
            {
                "use": "official",
                "family": "Smith",
                "given": ["Jane", "A."],
                "prefix": ["Ms."]
            },
            {"use": "maiden", "family": "Doe", "given": ["Jane"]}
        ],
        "telecom": [
            {"system": "phone", "value": "555-0100", "use": "home"},
            {"system": "phone", "value": "555-0199", "use": "mobile"},
            {"system": "email", "value": "jane.smith@example.org"}
        ],
        "gender": "female",
        "birthDate": "1980-01-15",
        "address": [
            {
                "use": "home",
                "type": "both",
                "line": ["123 Healthcare Ave", "Suite 400"],
                "city": "Boston",
                "state": "MA",
                "postalCode": "02115",
                "country": "USA",
                "period": {"start": "2010-01-01"}
            }
        ],
        "maritalStatus": {
            "coding": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/v3-MaritalStatus",
                    "code": "M",
                    "display": "Married"
                }
            ]
        },
        "communication": [
            {
                "language": {
                    "coding": [
                        {"system": "urn:ietf:bcp:47", "code": "en", "display": "English"}
                    ],
                    "text": "English"
                },
                "preferred": true
            }
        ],
        "generalPractitioner": [
            {
                "reference": "Practitioner/prac-999",
                "display": "Dr. Gregory House, MD"
            }
        ],
        "managingOrganization": {
            "reference": "Organization/org-001",
            "display": "MetroHealth Academic Medical Center"
        }
    })
});

pub static SYNTHETIC_OBSERVATION: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "resourceType": "Observation",
        "id": "obs-glucose-001",
        "meta": {
            "versionId": "1",
            "lastUpdated": "2026-03-31T08:30:00.000Z",
            "profile": [
                "http://hl7.org/fhir/us/core/StructureDefinition/us-core-observation-lab|3.1.1",
                "http://hl7.org/fhir/StructureDefinition/Observation"
            ],
            "source": "urn:uuid:lab-interface-gateway"
        },
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\"><div class=\"hapiHeaderText\"><b>Glucose in Blood</b></div><table class=\"hapiPropertyTable\"><tbody><tr><td>Status</td><td>FINAL</td></tr><tr><td>Code</td><td>15074-8 (LOINC)</td></tr><tr><td>Result Value</td><td>140 mg/dL</td></tr><tr><td>Interpretation</td><td>High (H)</td></tr><tr><td>Reference Range</td><td>70 - 99 mg/dL</td></tr><tr><td>Performer</td><td>Dr. Gregory House, Pathologist</td></tr><tr><td>Effective Date</td><td>2026-03-31 08:00:00 UTC</td></tr></tbody></table></div>"
        },
        "extension": [
            {
                "url": "http://example.org/fhir/StructureDefinition/lab-instrument-id",
                "valueString": "Roche-Cobas-8000-Unit-4-Serial-991283"
            },
            {
                "url": "http://hl7.org/fhir/StructureDefinition/observation-gatewayReceivedTime",
                "valueInstant": "2026-03-31T08:01:22.109Z"
            }
        ],
        "status": "final",
        "category": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/observation-category",
                        "code": "laboratory",
                        "display": "Laboratory"
                    }
                ],
                "text": "Laboratory"
            }
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "15074-8",
                    "display": "Glucose [Mass/volume] in Blood"
                },
                {
                    "system": "http://snomed.info/sct",
                    "code": "365645004",
                    "display": "Finding of glucose level in blood"
                },
                {
                    "system": "http://example.org/internal-lab-codes",
                    "code": "GLU-FASTING",
                    "display": "Glucose Fasting Plasma"
                }
            ],
            "text": "Fasting Blood Glucose"
        },
        "subject": {"reference": "Patient/pat-synthetic-001", "display": "Jane Smith"},
        "effectiveDateTime": "2026-03-31T08:00:00Z",
        "issued": "2026-03-31T08:25:00Z",
        "performer": [
            {
                "reference": "Practitioner/lab-tech-42",
                "display": "Dr. Gregory House, Clinical Pathologist"
            }
        ],
        "valueQuantity": {
            "value": 140,
            "unit": "mg/dL",
            "system": "http://unitsofmeasure.org",
            "code": "mg/dL"
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/v3-ObservationInterpretation",
                        "code": "H",
                        "display": "High"
                    }
                ],
                "text": "High"
            }
        ],
        "referenceRange": [
            {
                "low": {"value": 70, "unit": "mg/dL", "system": "http://unitsofmeasure.org"},
                "high": {"value": 99, "unit": "mg/dL", "system": "http://unitsofmeasure.org"},
                "type": {
                    "coding": [
                        {
                            "system": "http://terminology.hl7.org/CodeSystem/referencerange-meaning",
                            "code": "normal",
                            "display": "Normal Range"
                        }
                    ]
                },
                "text": "70 - 99 mg/dL"
            }
        ]
    })
});

pub static SYNTHETIC_CONDITION: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "resourceType": "Condition",
        "id": "cond-diabetes-001",
        "meta": {
            "versionId": "2",
            "lastUpdated": "2026-01-15T10:00:00.000Z",
            "profile": [
                "http://hl7.org/fhir/us/core/StructureDefinition/us-core-condition|3.1.1",
                "http://hl7.org/fhir/StructureDefinition/Condition"
            ],
            "tag": [{"system": "http://example.org/tags", "code": "chronic-disease-registry"}]
        },
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\"><div class=\"hapiHeaderText\"><b>Type 2 Diabetes Mellitus</b></div><table class=\"hapiPropertyTable\"><tbody><tr><td>Clinical Status</td><td>Active</td></tr><tr><td>Verification Status</td><td>Confirmed</td></tr><tr><td>Onset Date</td><td>15 January 2020</td></tr><tr><td>Coding</td><td>SNOMED: 44054006, ICD-10: E11.9</td></tr><tr><td>Physician</td><td>Dr. Gregory House, MD</td></tr><tr><td>Clinical Notes</td><td>Managed on oral biguanides.</td></tr></tbody></table></div>"
        },
        "extension": [
            {
                "url": "http://hl7.org/fhir/StructureDefinition/condition-assertedDate",
                "valueDateTime": "2020-01-15T10:30:00Z"
            },
            {
                "url": "http://example.org/fhir/StructureDefinition/hcc-risk-adjustment-category",
                "valueString": "HCC-18 (Diabetes with Chronic Complications)"
            }
        ],
        "clinicalStatus": {
            "coding": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/condition-clinical",
                    "code": "active",
                    "display": "Active"
                }
            ]
        },
        "verificationStatus": {
            "coding": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/condition-ver-status",
                    "code": "confirmed",
                    "display": "Confirmed"
                }
            ]
        },
        "category": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/condition-category",
                        "code": "problem-list-item",
                        "display": "Problem List Item"
                    }
                ],
                "text": "Problem List Item"
            }
        ],
        "severity": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "6736007",
                    "display": "Moderate"
                }
            ]
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "44054006",
                    "display": "Type 2 diabetes mellitus"
                },
                {
                    "system": "http://hl7.org/fhir/sid/icd-10-cm",
                    "code": "E11.9",
                    "display": "Type 2 diabetes mellitus without complications"
                },
                {
                    "system": "http://hl7.org/fhir/sid/icd-9-cm",
                    "code": "250.00",
                    "display": "Diabetes mellitus without mention of complication, type II"
                }
            ],
            "text": "Type 2 Diabetes Mellitus"
        },
        "subject": {"reference": "Patient/pat-synthetic-001", "display": "Jane Smith"},
        "onsetDateTime": "2020-01-15",
        "recordedDate": "2020-01-16",
        "recorder": {"reference": "Practitioner/prac-999", "display": "Dr. Gregory House"},
        "asserter": {"reference": "Practitioner/prac-999", "display": "Dr. Gregory House"},
        "note": [
            {
                "authorString": "Dr. Gregory House",
                "time": "2020-01-16T12:00:00Z",
                "text": "Patient exhibits classic symptoms with fasting plasma glucose > 126 mg/dL. Initiating Metformin therapy."
            }
        ]
    })
});

pub static SYNTHETIC_MEDICATION_REQUEST: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "resourceType": "MedicationRequest",
        "id": "med-metformin-001",
        "meta": {
            "versionId": "1",
            "lastUpdated": "2026-01-10T09:00:00.000Z",
            "profile": [
                "http://hl7.org/fhir/us/core/StructureDefinition/us-core-medicationrequest|3.1.1",
                "http://hl7.org/fhir/StructureDefinition/MedicationRequest"
            ]
        },
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\"><div class=\"hapiHeaderText\"><b>Metformin 500 MG Oral Tablet</b></div><table class=\"hapiPropertyTable\"><tbody><tr><td>Status</td><td>ACTIVE</td></tr><tr><td>Intent</td><td>Order</td></tr><tr><td>Prescriber</td><td>Dr. Gregory House, MD</td></tr><tr><td>Directions</td><td>Take 1 tablet orally daily with meals</td></tr><tr><td>Dispense</td><td>90 tablets, 3 refills</td></tr><tr><td>RxNorm</td><td>860975</td></tr></tbody></table></div>"
        },
        "extension": [
            {
                "url": "http://hl7.org/fhir/StructureDefinition/medicationrequest-treatmentIntent",
                "valueCodeableConcept": {
                    "coding": [
                        {
                            "system": "http://terminology.hl7.org/CodeSystem/treatment-intent",
                            "code": "curative",
                            "display": "Chronic Maintenance"
                        }
                    ]
                }
            }
        ],
        "status": "active",
        "intent": "order",
        "category": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/medicationrequest-category",
                        "code": "outpatient",
                        "display": "Outpatient"
                    }
                ]
            }
        ],
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "860975",
                    "display": "Metformin hydrochloride 500 MG Oral Tablet"
                },
                {
                    "system": "http://hl7.org/fhir/sid/ndc",
                    "code": "0093-1074-01",
                    "display": "Metformin HCl 500mg 100s Bottle"
                }
            ],
            "text": "Metformin 500mg tablet"
        },
        "subject": {"reference": "Patient/pat-synthetic-001", "display": "Jane Smith"},
        "authoredOn": "2026-01-10",
        "requester": {"reference": "Practitioner/prac-999", "display": "Dr. Gregory House"},
        "dosageInstruction": [
            {
                "sequence": 1,
                "text": "Take 1 tablet daily with meals",
                "additionalInstruction": [
                    {
                        "coding": [
                            {
                                "system": "http://snomed.info/sct",
                                "code": "311504000",
                                "display": "With or after food"
                            }
                        ]
                    }
                ],
                "timing": {
                    "repeat": {
                        "frequency": 1,
                        "period": 1,
                        "periodUnit": "d"
                    }
                },
                "route": {
                    "coding": [
                        {
                            "system": "http://snomed.info/sct",
                            "code": "260548002",
                            "display": "Oral"
                        }
                    ]
                },
                "doseAndRate": [
                    {
                        "type": {
                            "coding": [
                                {
                                    "system": "http://terminology.hl7.org/CodeSystem/dose-rate-type",
                                    "code": "ordered",
                                    "display": "Ordered"
                                }
                            ]
                        },
                        "doseQuantity": {
                            "value": 1,
                            "unit": "tablet",
                            "system": "http://terminology.hl7.org/CodeSystem/v3-orderableDrugForm",
                            "code": "TAB"
                        }
                    }
                ]
            }
        ],
        "dispenseRequest": {
            "numberOfRepeatsAllowed": 3,
            "quantity": {"value": 90, "unit": "tablets"},
            "expectedSupplyDuration": {"value": 90, "unit": "days"},
            "performer": {
                "reference": "Organization/pharmacy-001",
                "display": "MetroHealth Outpatient Pharmacy"
            }
        },
        "substitution": {
            "allowedBoolean": true,
            "reason": {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/v3-ActReason",
                        "code": "FP",
                        "display": "Formulary Policy"
                    }
                ]
            }
        }
    })
});

pub static SYNTHETIC_ALLERGY_INTOLERANCE: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "resourceType": "AllergyIntolerance",
        "id": "allergy-pnc-001",
        "meta": {
            "versionId": "1",
            "lastUpdated": "2025-05-20T14:00:00.000Z",
            "profile": [
                "http://hl7.org/fhir/us/core/StructureDefinition/us-core-allergyintolerance|3.1.1",
                "http://hl7.org/fhir/StructureDefinition/AllergyIntolerance"
            ]
        },
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\"><div class=\"hapiHeaderText\"><b>Severe Penicillin Allergy</b></div><table class=\"hapiPropertyTable\"><tbody><tr><td>Criticality</td><td>HIGH</td></tr><tr><td>Status</td><td>Active (Confirmed)</td></tr><tr><td>Substance</td><td>Penicillin</td></tr><tr><td>Manifestation</td><td>Anaphylactic Shock, Bronchospasm</td></tr><tr><td>Reported Date</td><td>20 May 2015</td></tr><tr><td>Recorder</td><td>Dr. Gregory House, MD</td></tr><tr><td>Caution</td><td>Strict contraindication to beta-lactams.</td></tr></tbody></table></div>"
        },
        "extension": [
            {
                "url": "http://hl7.org/fhir/StructureDefinition/allergyintolerance-certainty",
                "valueCodeableConcept": {
                    "coding": [
                        {
                            "system": "http://terminology.hl7.org/CodeSystem/reaction-event-certainty",
                            "code": "confirmed",
                            "display": "Confirmed by clinical challenge"
                        }
                    ]
                }
            },
            {
                "url": "http://example.org/fhir/StructureDefinition/allergy-alert-banner",
                "valueBoolean": true
            }
        ],
        "clinicalStatus": {
            "coding": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/allergyintolerance-clinical",
                    "code": "active",
                    "display": "Active"
                }
            ]
        },
        "verificationStatus": {
            "coding": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/allergyintolerance-verification",
                    "code": "confirmed",
                    "display": "Confirmed"
                }
            ]
        },
        "type": "allergy",
        "category": ["medication"],
        "criticality": "high",
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "764146007",
                    "display": "Penicillin (substance)"
                },
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "7980",
                    "display": "Penicillin"
                },
                {
                    "system": "http://fdasis.nlm.nih.gov",
                    "code": "Q42T66VG0C",
                    "display": "Penicillin G"
                }
            ],
            "text": "Penicillin allergy"
        },
        "patient": {"reference": "Patient/pat-synthetic-001", "display": "Jane Smith"},
        "onsetDateTime": "2015-05-20",
        "recordedDate": "2015-05-21",
        "recorder": {"reference": "Practitioner/prac-999", "display": "Dr. Gregory House"},
        "lastOccurrence": "2015-05-20T11:45:00Z",
        "note": [
            {
                "authorString": "Emergency Dept Triage",
                "time": "2015-05-20T13:00:00Z",
                "text": "Administered IV penicillin G resulted in acute respiratory distress within 10 minutes. Responded to epinephrine."
            }
        ],
        "reaction": [
            {
                "substance": {
                    "coding": [
                        {
                            "system": "http://snomed.info/sct",
                            "code": "764146007",
                            "display": "Penicillin (substance)"
                        }
                    ]
                },
                "manifestation": [
                    {
                        "coding": [
                            {
                                "system": "http://snomed.info/sct",
                                "code": "39579001",
                                "display": "Anaphylaxis"
                            }
                        ],
                        "text": "Anaphylaxis"
                    },
                    {
                        "coding": [
                            {
                                "system": "http://snomed.info/sct",
                                "code": "247472004",
                                "display": "Wheal and erythema"
                            }
                        ],
                        "text": "Diffuse urticaria"
                    }
                ],
                "description": "Patient developed acute anaphylaxis requiring IM epinephrine and airway management.",
                "onset": "2015-05-20T11:50:00Z",
                "severity": "severe",
                "exposureRoute": {
                    "coding": [
                        {
                            "system": "http://snomed.info/sct",
                            "code": "47625008",
                            "display": "Intravenous route"
                        }
                    ]
                }
            }
        ]
    })
});

pub static SYNTHETIC_DIAGNOSTIC_REPORT: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "resourceType": "DiagnosticReport",
        "id": "diag-report-001",
        "meta": {
            "versionId": "1",
            "lastUpdated": "2026-03-31T11:00:00.000Z",
            "profile": [
                "http://hl7.org/fhir/us/core/StructureDefinition/us-core-diagnosticreport-lab|3.1.1",
                "http://hl7.org/fhir/StructureDefinition/DiagnosticReport"
            ]
        },
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\"><div class=\"hapiHeaderText\"><b>Comprehensive Metabolic Panel</b></div><table class=\"hapiPropertyTable\"><tbody><tr><td>Status</td><td>FINAL</td></tr><tr><td>Category</td><td>Laboratory (LAB)</td></tr><tr><td>Diagnostic Service</td><td>Biochemistry</td></tr><tr><td>Effective Date</td><td>2026-03-31 09:00:00 UTC</td></tr><tr><td>Conclusion</td><td>Elevated fasting glucose.</td></tr><tr><td>Lab Director</td><td>Dr. Gregory House, Clinical Pathologist</td></tr><tr><td>Accreditation</td><td>CAP / CLIA #99D102938</td></tr></tbody></table></div>"
        },
        "extension": [
            {
                "url": "http://example.org/fhir/StructureDefinition/lab-turnaround-time",
                "valueQuantity": {"value": 110, "unit": "min"}
            }
        ],
        "identifier": [
            {
                "system": "http://example.org/lab-orders",
                "value": "ORD-2026-0331-LAB-8819"
            }
        ],
        "status": "final",
        "category": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/v2-0074",
                        "code": "LAB",
                        "display": "Laboratory"
                    }
                ],
                "text": "Laboratory"
            }
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "24323-8",
                    "display": "Comprehensive Metabolic 2000 Panel - Serum or Plasma"
                },
                {
                    "system": "http://example.org/cpt",
                    "code": "80053",
                    "display": "Comprehensive metabolic panel"
                }
            ],
            "text": "Comprehensive Metabolic Panel"
        },
        "subject": {"reference": "Patient/pat-synthetic-001", "display": "Jane Smith"},
        "encounter": {"reference": "Encounter/enc-annual-001", "display": "Routine Annual Checkup"},
        "effectiveDateTime": "2026-03-31T09:00:00Z",
        "issued": "2026-03-31T11:00:00Z",
        "performer": [
            {
                "reference": "Organization/lab-central",
                "display": "Central Diagnostic Clinical Lab"
            },
            {
                "reference": "Practitioner/prac-999",
                "display": "Dr. Gregory House, Pathologist"
            }
        ],
        "specimen": [
            {
                "reference": "Specimen/spec-serum-001",
                "display": "Venous blood specimen drawn 2026-03-31 08:45:00 UTC"
            }
        ],
        "result": [
            {"reference": "Observation/obs-glucose-001", "display": "Glucose: 140 mg/dL (H)"},
            {"reference": "Observation/obs-na-002", "display": "Sodium: 139 mEq/L"},
            {"reference": "Observation/obs-k-003", "display": "Potassium: 4.2 mEq/L"},
            {"reference": "Observation/obs-cl-004", "display": "Chloride: 102 mEq/L"},
            {"reference": "Observation/obs-co2-005", "display": "Carbon Dioxide: 24 mEq/L"},
            {"reference": "Observation/obs-bun-006", "display": "Blood Urea Nitrogen: 16 mg/dL"},
            {"reference": "Observation/obs-cr-007", "display": "Creatinine: 0.9 mg/dL"}
        ],
        "conclusion": "Elevated fasting glucose, consistent with impaired fasting glycemia.",
        "conclusionCode": [
            {
                "coding": [
                    {
                        "system": "http://snomed.info/sct",
                        "code": "237599002",
                        "display": "Impaired fasting glucose"
                    }
                ]
            }
        ]
    })
});

pub static SYNTHETIC_ENCOUNTER: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "resourceType": "Encounter",
        "id": "enc-annual-001",
        "meta": {
            "versionId": "1",
            "lastUpdated": "2026-03-31T10:00:00.000Z",
            "profile": [
                "http://hl7.org/fhir/us/core/StructureDefinition/us-core-encounter|3.1.1",
                "http://hl7.org/fhir/StructureDefinition/Encounter"
            ]
        },
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\"><div class=\"hapiHeaderText\"><b>Encounter: Routine Annual Checkup</b></div><table class=\"hapiPropertyTable\"><tbody><tr><td>Status</td><td>FINISHED</td></tr><tr><td>Class</td><td>Ambulatory (AMB)</td></tr><tr><td>Period</td><td>2026-03-31 08:00 - 09:00 UTC</td></tr><tr><td>Reason</td><td>Routine general medical examination</td></tr><tr><td>Practitioner</td><td>Dr. Gregory House, MD</td></tr><tr><td>Facility</td><td>Main Hospital Health Clinic</td></tr><tr><td>Billing Account</td><td>ACCT-99214-AMB</td></tr></tbody></table></div>"
        },
        "extension": [
            {
                "url": "http://example.org/fhir/StructureDefinition/encounter-acuity",
                "valueString": "Level-3 Standard Ambulatory Visit"
            },
            {
                "url": "http://hl7.org/fhir/StructureDefinition/encounter-modeOfArrival",
                "valueCoding": {
                    "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
                    "code": "WALK",
                    "display": "Walk-in patient"
                }
            }
        ],
        "identifier": [
            {
                "use": "official",
                "system": "http://example.org/encounters",
                "value": "ENC-20260331-01928"
            }
        ],
        "status": "finished",
        "class": {
            "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
            "code": "AMB",
            "display": "ambulatory"
        },
        "type": [
            {
                "coding": [
                    {
                        "system": "http://snomed.info/sct",
                        "code": "185349003",
                        "display": "Encounter for check up"
                    },
                    {
                        "system": "http://example.org/cpt",
                        "code": "99214",
                        "display": "Office or other outpatient visit, 30-39 minutes"
                    }
                ],
                "text": "Routine checkup"
            }
        ],
        "subject": {"reference": "Patient/pat-synthetic-001", "display": "Jane Smith"},
        "participant": [
            {
                "type": [
                    {
                        "coding": [
                            {
                                "system": "http://terminology.hl7.org/CodeSystem/v3-ParticipationType",
                                "code": "PPRF",
                                "display": "primary performer"
                            }
                        ]
                    }
                ],
                "individual": {
                    "reference": "Practitioner/prac-999",
                    "display": "Dr. Gregory House, MD"
                }
            }
        ],
        "period": {
            "start": "2026-03-31T08:00:00Z",
            "end": "2026-03-31T09:00:00Z"
        },
        "reasonCode": [
            {
                "coding": [
                    {
                        "system": "http://snomed.info/sct",
                        "code": "410620009",
                        "display": "Well person health check"
                    },
                    {
                        "system": "http://hl7.org/fhir/sid/icd-10-cm",
                        "code": "Z00.00",
                        "display": "Encounter for general adult examination"
                    }
                ],
                "text": "Routine general medical examination"
            }
        ],
        "serviceProvider": {
            "reference": "Organization/org-001",
            "display": "Main Hospital Health Clinic"
        }
    })
});

pub static SYNTHETIC_CARE_PLAN: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "resourceType": "CarePlan",
        "id": "cp-diabetes-001",
        "meta": {
            "versionId": "1",
            "lastUpdated": "2026-01-01T08:00:00.000Z",
            "profile": [
                "http://hl7.org/fhir/us/core/StructureDefinition/us-core-careplan|3.1.1",
                "http://hl7.org/fhir/StructureDefinition/CarePlan"
            ]
        },
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\"><div class=\"hapiHeaderText\"><b>Diabetes Management Plan</b></div><table class=\"hapiPropertyTable\"><tbody><tr><td>Status</td><td>ACTIVE</td></tr><tr><td>Intent</td><td>Plan</td></tr><tr><td>Care Coordinator</td><td>Dr. Gregory House, MD</td></tr><tr><td>Goals</td><td>Maintain HbA1c < 7.0%</td></tr><tr><td>Activities</td><td>Dietary consultation, exercise</td></tr><tr><td>Review Frequency</td><td>Quarterly evaluation</td></tr></tbody></table></div>"
        },
        "extension": [
            {
                "url": "http://example.org/fhir/StructureDefinition/careplan-risk-stratification",
                "valueString": "Moderate Chronic Complexity Tier 2"
            },
            {
                "url": "http://example.org/fhir/StructureDefinition/careplan-telehealth-eligible",
                "valueBoolean": true
            }
        ],
        "status": "active",
        "intent": "plan",
        "title": "Diabetes Management Plan",
        "description": "Evidence-based comprehensive ambulatory diabetes management and lifestyle optimization protocol.",
        "subject": {"reference": "Patient/pat-synthetic-001", "display": "Jane Smith"},
        "period": {
            "start": "2026-01-01",
            "end": "2026-12-31"
        },
        "author": {"reference": "Practitioner/prac-999", "display": "Dr. Gregory House, MD"},
        "goal": [
            {
                "reference": "Goal/goal-a1c-001",
                "display": "Maintain target HbA1c below 7.0 percent"
            },
            {
                "reference": "Goal/goal-bp-002",
                "display": "Maintain systolic blood pressure under 130 mmHg"
            }
        ],
        "activity": [
            {
                "detail": {
                    "kind": "Appointment",
                    "code": {
                        "coding": [
                            {
                                "system": "http://snomed.info/sct",
                                "code": "103699006",
                                "display": "Dietary consultation"
                            }
                        ],
                        "text": "Dietary consultation"
                    },
                    "status": "in-progress",
                    "scheduledPeriod": {"start": "2026-02-01", "end": "2026-02-01"},
                    "performer": [{"display": "Clinical Nutrition Specialist"}],
                    "description": "Medical nutrition therapy and carbohydrate counting."
                }
            },
            {
                "detail": {
                    "kind": "Procedure",
                    "code": {
                        "coding": [
                            {
                                "system": "http://snomed.info/sct",
                                "code": "359771000000103",
                                "display": "Self-monitoring of blood glucose"
                            }
                        ],
                        "text": "Daily blood glucose monitoring"
                    },
                    "status": "in-progress",
                    "description": "Daily fasting and 2-hour postprandial blood glucose log review."
                }
            },
            {
                "detail": {
                    "kind": "Procedure",
                    "code": {
                        "coding": [
                            {
                                "system": "http://snomed.info/sct",
                                "code": "229065009",
                                "display": "Exercise regimen"
                            }
                        ],
                        "text": "Exercise regimen"
                    },
                    "status": "in-progress",
                    "description": "Moderate intensity aerobic cardiovascular exercise at least 150 min/week."
                }
            }
        ]
    })
});

pub static SYNTHETIC_BUNDLE: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "resourceType": "Bundle",
        "id": "bundle-patient-chart-001",
        "meta": {
            "versionId": "1",
            "lastUpdated": "2026-03-31T12:00:00.000Z"
        },
        "type": "searchset",
        "total": 8,
        "entry": [
            {"fullUrl": "urn:uuid:pat-synthetic-001", "resource": *SYNTHETIC_PATIENT},
            {"fullUrl": "urn:uuid:obs-glucose-001", "resource": *SYNTHETIC_OBSERVATION},
            {"fullUrl": "urn:uuid:cond-diabetes-001", "resource": *SYNTHETIC_CONDITION},
            {"fullUrl": "urn:uuid:med-metformin-001", "resource": *SYNTHETIC_MEDICATION_REQUEST},
            {"fullUrl": "urn:uuid:allergy-pnc-001", "resource": *SYNTHETIC_ALLERGY_INTOLERANCE},
            {"fullUrl": "urn:uuid:diag-report-001", "resource": *SYNTHETIC_DIAGNOSTIC_REPORT},
            {"fullUrl": "urn:uuid:enc-annual-001", "resource": *SYNTHETIC_ENCOUNTER},
            {"fullUrl": "urn:uuid:cp-diabetes-001", "resource": *SYNTHETIC_CARE_PLAN}
        ]
    })
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub resource_type: String,
    pub raw_bytes: usize,
    pub compact_bytes: usize,
    pub standard_bytes: usize,
    pub executive_bytes: usize,
    pub raw_tokens: usize,
    pub compact_tokens: usize,
    pub standard_tokens: usize,
    pub executive_tokens: usize,
    pub compact_reduction_pct: f64,
    pub standard_reduction_pct: f64,
    pub executive_reduction_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateBenchmarkReport {
    pub results: HashMap<String, BenchmarkResult>,
    pub total_raw_bytes: usize,
    pub total_compact_bytes: usize,
    pub total_standard_bytes: usize,
    pub total_executive_bytes: usize,
    pub total_raw_tokens: usize,
    pub total_compact_tokens: usize,
    pub total_standard_tokens: usize,
    pub total_executive_tokens: usize,
    pub aggregate_compact_reduction_pct: f64,
    pub aggregate_standard_reduction_pct: f64,
    pub aggregate_executive_reduction_pct: f64,
    pub overall_reduction_pct: f64,
}

/// Run token distillation benchmark suite across synthetic fixtures.
pub fn run_fhir_benchmarks() -> Result<AggregateBenchmarkReport, String> {
    let fixtures: [(&str, &Value); 9] = [
        ("Patient", &SYNTHETIC_PATIENT),
        ("Observation", &SYNTHETIC_OBSERVATION),
        ("Condition", &SYNTHETIC_CONDITION),
        ("MedicationRequest", &SYNTHETIC_MEDICATION_REQUEST),
        ("AllergyIntolerance", &SYNTHETIC_ALLERGY_INTOLERANCE),
        ("DiagnosticReport", &SYNTHETIC_DIAGNOSTIC_REPORT),
        ("Encounter", &SYNTHETIC_ENCOUNTER),
        ("CarePlan", &SYNTHETIC_CARE_PLAN),
        ("Bundle", &SYNTHETIC_BUNDLE),
    ];

    let mut results = HashMap::new();

    let mut total_raw_bytes = 0;
    let mut total_compact_bytes = 0;
    let mut total_standard_bytes = 0;
    let mut total_executive_bytes = 0;

    let mut total_raw_tokens = 0;
    let mut total_compact_tokens = 0;
    let mut total_standard_tokens = 0;
    let mut total_executive_tokens = 0;

    for (name, fixture) in fixtures {
        let raw_b = serialize_bytes(fixture);
        let raw_tok = estimate_tokens(fixture);

        let compact_res = distill_resource(fixture, DetailLevel::Compact);
        let compact_b = serialize_bytes(&compact_res);
        let compact_tok = estimate_tokens(&compact_res);

        let standard_res = distill_resource(fixture, DetailLevel::Standard);
        let standard_b = serialize_bytes(&standard_res);
        let standard_tok = estimate_tokens(&standard_res);

        let executive_res = distill_resource(fixture, DetailLevel::Executive);
        let executive_b = serialize_bytes(&executive_res);
        let executive_tok = estimate_tokens(&executive_res);

        let c_red = (1.0 - (compact_tok as f64 / raw_tok as f64)) * 100.0;
        let s_red = (1.0 - (standard_tok as f64 / raw_tok as f64)) * 100.0;
        let e_red = (1.0 - (executive_tok as f64 / raw_tok as f64)) * 100.0;

        results.insert(
            name.to_string(),
            BenchmarkResult {
                resource_type: name.to_string(),
                raw_bytes: raw_b,
                compact_bytes: compact_b,
                standard_bytes: standard_b,
                executive_bytes: executive_b,
                raw_tokens: raw_tok,
                compact_tokens: compact_tok,
                standard_tokens: standard_tok,
                executive_tokens: executive_tok,
                compact_reduction_pct: (c_red * 100.0).round() / 100.0,
                standard_reduction_pct: (s_red * 100.0).round() / 100.0,
                executive_reduction_pct: (e_red * 100.0).round() / 100.0,
            },
        );

        total_raw_bytes += raw_b;
        total_compact_bytes += compact_b;
        total_standard_bytes += standard_b;
        total_executive_bytes += executive_b;

        total_raw_tokens += raw_tok;
        total_compact_tokens += compact_tok;
        total_standard_tokens += standard_tok;
        total_executive_tokens += executive_tok;
    }

    let agg_c_red = (1.0 - (total_compact_tokens as f64 / total_raw_tokens as f64)) * 100.0;
    let agg_s_red = (1.0 - (total_standard_tokens as f64 / total_raw_tokens as f64)) * 100.0;
    let agg_e_red = (1.0 - (total_executive_tokens as f64 / total_raw_tokens as f64)) * 100.0;
    let overall_reduction = (agg_c_red + agg_s_red + agg_e_red) / 3.0;

    if overall_reduction < 85.0 {
        return Err(format!(
            "Aggregate overall token reduction {overall_reduction:.2}% is below target 85.0%"
        ));
    }

    Ok(AggregateBenchmarkReport {
        results,
        total_raw_bytes,
        total_compact_bytes,
        total_standard_bytes,
        total_executive_bytes,
        total_raw_tokens,
        total_compact_tokens,
        total_standard_tokens,
        total_executive_tokens,
        aggregate_compact_reduction_pct: (agg_c_red * 100.0).round() / 100.0,
        aggregate_standard_reduction_pct: (agg_s_red * 100.0).round() / 100.0,
        aggregate_executive_reduction_pct: (agg_e_red * 100.0).round() / 100.0,
        overall_reduction_pct: (overall_reduction * 100.0).round() / 100.0,
    })
}
