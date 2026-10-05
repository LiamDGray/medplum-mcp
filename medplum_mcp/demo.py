"""In-Memory Clinical Sandbox with St. Jude Research Institute Synthetic Dataset.

Provides realistic FHIR R4 clinical data and deterministic query/draft creation methods.
"""

from __future__ import annotations

import copy
import uuid
from typing import Any

from medplum_mcp.safety import assert_write_permitted

_PATIENTS: list[dict[str, Any]] = [
    {
        "resourceType": "Patient",
        "id": "pat-sj-001",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100234",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Vance",
                "given": [
                    "Caleb",
                    "Michael",
                ],
                "text": "Caleb Michael Vance",
            },
        ],
        "telecom": [
            {
                "system": "phone",
                "value": "555-014-8821",
                "use": "home",
            },
            {
                "system": "email",
                "value": "parents.vance@example.org",
            },
        ],
        "gender": "male",
        "birthDate": "2019-04-12",
        "address": [
            {
                "use": "home",
                "line": [
                    "262 Danny Thomas Place",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38105",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-002",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100235",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Lin",
                "given": [
                    "Maya",
                    "Sophia",
                ],
                "text": "Maya Sophia Lin",
            },
        ],
        "gender": "female",
        "birthDate": "2015-08-20",
        "address": [
            {
                "use": "home",
                "line": [
                    "450 Hospital Blvd",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38105",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-003",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100236",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Washington",
                "given": [
                    "Dante",
                    "Jamal",
                ],
                "text": "Dante Jamal Washington",
            },
        ],
        "gender": "male",
        "birthDate": "2012-01-15",
        "address": [
            {
                "use": "home",
                "line": [
                    "812 Beale Street",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38103",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-004",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100237",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Kowalski",
                "given": [
                    "Sophia",
                    "Rose",
                ],
                "text": "Sophia Rose Kowalski",
            },
        ],
        "gender": "female",
        "birthDate": "2022-06-03",
        "address": [
            {
                "use": "home",
                "line": [
                    "104 Riverfront Dr",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38103",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-005",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100238",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "O'Connor",
                "given": [
                    "Liam",
                    "Patrick",
                ],
                "text": "Liam Patrick O'Connor",
            },
        ],
        "gender": "male",
        "birthDate": "2010-11-28",
        "address": [
            {
                "use": "home",
                "line": [
                    "318 Poplar Ave",
                ],
                "city": "Germantown",
                "state": "TN",
                "postalCode": "38138",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-006",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100239",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Al-Mansoor",
                "given": [
                    "Aisha",
                ],
                "text": "Aisha Al-Mansoor",
            },
        ],
        "gender": "female",
        "birthDate": "2024-03-09",
        "address": [
            {
                "use": "home",
                "line": [
                    "190 Overton Ave",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38105",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-007",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100240",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Silva",
                "given": [
                    "Lucas",
                    "Gabriel",
                ],
                "text": "Lucas Gabriel Silva",
            },
        ],
        "gender": "male",
        "birthDate": "2017-07-14",
        "address": [
            {
                "use": "home",
                "line": [
                    "525 Cooper St",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38104",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-008",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100241",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Bennett",
                "given": [
                    "Chloe",
                    "Grace",
                ],
                "text": "Chloe Grace Bennett",
            },
        ],
        "gender": "female",
        "birthDate": "2023-09-22",
        "address": [
            {
                "use": "home",
                "line": [
                    "640 Union Ave",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38103",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-009",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100242",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Zhang",
                "given": [
                    "Ethan",
                    "Wei",
                ],
                "text": "Ethan Wei Zhang",
            },
        ],
        "gender": "male",
        "birthDate": "2013-05-18",
        "address": [
            {
                "use": "home",
                "line": [
                    "720 Highland St",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38111",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-010",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100243",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Tremblay",
                "given": [
                    "Zoe",
                    "Claire",
                ],
                "text": "Zoe Claire Tremblay",
            },
        ],
        "gender": "female",
        "birthDate": "2020-10-05",
        "address": [
            {
                "use": "home",
                "line": [
                    "915 Park Ave",
                ],
                "city": "Collierville",
                "state": "TN",
                "postalCode": "38017",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-011",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100244",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Patel",
                "given": [
                    "Noah",
                    "Dev",
                ],
                "text": "Noah Dev Patel",
            },
        ],
        "gender": "male",
        "birthDate": "2018-02-11",
        "address": [
            {
                "use": "home",
                "line": [
                    "410 Main St",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38103",
            },
        ],
    },
    {
        "resourceType": "Patient",
        "id": "pat-sj-012",
        "identifier": [
            {
                "system": "http://hospital.smarthealthit.org",
                "value": "MRN-SJ-100245",
                "type": {
                    "text": "MRN",
                },
            },
        ],
        "active": True,
        "name": [
            {
                "use": "official",
                "family": "Martinez",
                "given": [
                    "Isabella",
                    "Marie",
                ],
                "text": "Isabella Marie Martinez",
            },
        ],
        "gender": "female",
        "birthDate": "2011-12-30",
        "address": [
            {
                "use": "home",
                "line": [
                    "1500 Midtown Blvd",
                ],
                "city": "Memphis",
                "state": "TN",
                "postalCode": "38104",
            },
        ],
    },
]

_OBSERVATIONS: list[dict[str, Any]] = [
    {
        "resourceType": "Observation",
        "id": "obs-sj-001",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/observation-category",
                        "code": "laboratory",
                        "display": "Laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "26499-4",
                    "display": "Absolute Neutrophil Count",
                },
            ],
            "text": "Absolute Neutrophil Count",
        },
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "effectiveDateTime": "2026-04-01T08:30:00Z",
        "valueQuantity": {
            "value": 450,
            "unit": "/uL",
            "system": "http://unitsofmeasure.org",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "L",
                        "display": "Low",
                    },
                ],
            },
        ],
        "referenceRange": [
            {
                "low": {
                    "value": 1500,
                },
                "high": {
                    "value": 8000,
                },
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-002",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "777-3",
                    "display": "Platelets [#/volume] in Blood",
                },
            ],
            "text": "Platelet Count",
        },
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "effectiveDateTime": "2026-04-01T08:30:00Z",
        "valueQuantity": {
            "value": 42000,
            "unit": "/uL",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "L",
                        "display": "Low",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-003",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "vital-signs",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "8310-5",
                    "display": "Body temperature",
                },
            ],
            "text": "Body Temperature",
        },
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "effectiveDateTime": "2026-04-01T09:00:00Z",
        "valueQuantity": {
            "value": 38.6,
            "unit": "Cel",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "H",
                        "display": "High (Febrile)",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-004",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "10331-7",
                    "display": "CSF Cytology",
                },
            ],
            "text": "Cerebrospinal Fluid Cytology",
        },
        "subject": {
            "reference": "Patient/pat-sj-002",
        },
        "effectiveDateTime": "2026-03-28T14:15:00Z",
        "valueString": "No malignant cells identified (M0 status)",
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-005",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "718-7",
                    "display": "Hemoglobin [Mass/volume]",
                },
            ],
            "text": "Hemoglobin",
        },
        "subject": {
            "reference": "Patient/pat-sj-003",
        },
        "effectiveDateTime": "2026-03-30T10:00:00Z",
        "valueQuantity": {
            "value": 7.4,
            "unit": "g/dL",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "L",
                        "display": "Low",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-006",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "20570-8",
                    "display": "Hemoglobin S/Total Hgb",
                },
            ],
            "text": "Hemoglobin S Fraction",
        },
        "subject": {
            "reference": "Patient/pat-sj-003",
        },
        "effectiveDateTime": "2026-03-30T10:00:00Z",
        "valueQuantity": {
            "value": 58.2,
            "unit": "%",
        },
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-007",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "3046-6",
                    "display": "Vanillylmandelic acid (VMA)",
                },
            ],
            "text": "Urine VMA",
        },
        "subject": {
            "reference": "Patient/pat-sj-004",
        },
        "effectiveDateTime": "2026-03-25T11:00:00Z",
        "valueQuantity": {
            "value": 24.5,
            "unit": "mg/24h",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "H",
                        "display": "High",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-008",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "6768-6",
                    "display": "Alkaline phosphatase",
                },
            ],
            "text": "Serum Alkaline Phosphatase",
        },
        "subject": {
            "reference": "Patient/pat-sj-005",
        },
        "effectiveDateTime": "2026-03-27T08:00:00Z",
        "valueQuantity": {
            "value": 480,
            "unit": "U/L",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "H",
                        "display": "High",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-009",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "8123-2",
                    "display": "CD3+ T-Cells",
                },
            ],
            "text": "Absolute CD3+ T Lymphocytes",
        },
        "subject": {
            "reference": "Patient/pat-sj-006",
        },
        "effectiveDateTime": "2026-03-15T09:30:00Z",
        "valueQuantity": {
            "value": 1100,
            "unit": "/uL",
        },
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-010",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "30341-2",
                    "display": "Erythrocyte sedimentation rate",
                },
            ],
            "text": "ESR",
        },
        "subject": {
            "reference": "Patient/pat-sj-007",
        },
        "effectiveDateTime": "2026-03-20T10:15:00Z",
        "valueQuantity": {
            "value": 45,
            "unit": "mm/h",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "H",
                        "display": "High",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-011",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "exam",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "252800008",
                    "display": "Fundoscopy",
                },
            ],
            "text": "Ophthalmologic Fundoscopic Exam",
        },
        "subject": {
            "reference": "Patient/pat-sj-008",
        },
        "effectiveDateTime": "2026-03-12T13:00:00Z",
        "valueString": ("Bilateral calcified retinal tumors, macula spared in right eye"),
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-012",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "26477-0",
                    "display": "Absolute Reticulocyte Count",
                },
            ],
            "text": "Absolute Reticulocyte Count",
        },
        "subject": {
            "reference": "Patient/pat-sj-009",
        },
        "effectiveDateTime": "2026-03-22T08:45:00Z",
        "valueQuantity": {
            "value": 15000,
            "unit": "/uL",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "L",
                        "display": "Severely Depressed",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-013",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "1994-3",
                    "display": "Sweat Chloride",
                },
            ],
            "text": "Sweat Chloride Concentration",
        },
        "subject": {
            "reference": "Patient/pat-sj-010",
        },
        "effectiveDateTime": "2026-02-18T10:00:00Z",
        "valueQuantity": {
            "value": 92,
            "unit": "mmol/L",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "H",
                        "display": "Consistent with CF",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-014",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "6690-2",
                    "display": "Leukocytes [#/volume] in Blood",
                },
            ],
            "text": "White Blood Cell Count",
        },
        "subject": {
            "reference": "Patient/pat-sj-011",
        },
        "effectiveDateTime": "2026-03-29T07:30:00Z",
        "valueQuantity": {
            "value": 85000,
            "unit": "/uL",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "H",
                        "display": "Hyperleukocytosis",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-015",
        "status": "preliminary",
        "category": [
            {
                "coding": [
                    {
                        "code": "laboratory",
                    },
                ],
            },
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "2532-0",
                    "display": "Lactate dehydrogenase",
                },
            ],
            "text": "Serum LDH",
        },
        "subject": {
            "reference": "Patient/pat-sj-012",
        },
        "effectiveDateTime": "2026-03-31T09:00:00Z",
        "valueQuantity": {
            "value": 620,
            "unit": "U/L",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "H",
                        "display": "Elevated",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-016",
        "status": "preliminary",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "2160-0",
                    "display": "Creatinine",
                },
            ],
            "text": "Serum Creatinine",
        },
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "effectiveDateTime": "2026-04-01T08:30:00Z",
        "valueQuantity": {
            "value": 0.45,
            "unit": "mg/dL",
        },
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-017",
        "status": "preliminary",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "1751-7",
                    "display": "Albumin",
                },
            ],
            "text": "Serum Albumin",
        },
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "effectiveDateTime": "2026-04-01T08:30:00Z",
        "valueQuantity": {
            "value": 3.8,
            "unit": "g/dL",
        },
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-018",
        "status": "preliminary",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "2276-4",
                    "display": "Ferritin",
                },
            ],
            "text": "Serum Ferritin",
        },
        "subject": {
            "reference": "Patient/pat-sj-003",
        },
        "effectiveDateTime": "2026-03-30T10:00:00Z",
        "valueQuantity": {
            "value": 1850,
            "unit": "ng/mL",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "H",
                        "display": "High (Iron Overload)",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-019",
        "status": "preliminary",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "2951-2",
                    "display": "Sodium",
                },
            ],
            "text": "Serum Sodium",
        },
        "subject": {
            "reference": "Patient/pat-sj-002",
        },
        "effectiveDateTime": "2026-03-28T14:15:00Z",
        "valueQuantity": {
            "value": 139,
            "unit": "mmol/L",
        },
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-020",
        "status": "preliminary",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "2823-3",
                    "display": "Potassium",
                },
            ],
            "text": "Serum Potassium",
        },
        "subject": {
            "reference": "Patient/pat-sj-004",
        },
        "effectiveDateTime": "2026-03-25T11:00:00Z",
        "valueQuantity": {
            "value": 4.1,
            "unit": "mmol/L",
        },
    },
    {
        "resourceType": "Observation",
        "id": "obs-sj-021",
        "status": "preliminary",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "1975-2",
                    "display": "Bilirubin Total",
                },
            ],
            "text": "Total Bilirubin",
        },
        "subject": {
            "reference": "Patient/pat-sj-003",
        },
        "effectiveDateTime": "2026-03-30T10:00:00Z",
        "valueQuantity": {
            "value": 2.8,
            "unit": "mg/dL",
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "code": "H",
                        "display": "Elevated (Hemolysis)",
                    },
                ],
            },
        ],
    },
]

_CONDITIONS: list[dict[str, Any]] = [
    {
        "resourceType": "Condition",
        "id": "cond-sj-001",
        "clinicalStatus": {
            "coding": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/condition-clinical",
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/condition-ver-status",
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "109989006",
                    "display": "B-cell acute lymphoblastic leukemia",
                },
            ],
            "text": ("B-Cell Acute Lymphoblastic Leukemia (B-ALL, ETV6-RUNX1 positive)"),
        },
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "onsetDateTime": "2025-11-10",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-002",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "443333004",
                    "display": "Medulloblastoma",
                },
            ],
            "text": "WNT-Subgroup Medulloblastoma (Posterior Fossa)",
        },
        "subject": {
            "reference": "Patient/pat-sj-002",
        },
        "onsetDateTime": "2025-09-14",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-003",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "127040003",
                    "display": "Sickle cell anemia",
                },
            ],
            "text": ("Sickle Cell Disease (HbSS) with Recurrent Vaso-Occlusive Crises"),
        },
        "subject": {
            "reference": "Patient/pat-sj-003",
        },
        "onsetDateTime": "2012-02-01",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-004",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "432148006",
                    "display": "Neuroblastoma",
                },
            ],
            "text": "High-Risk Neuroblastoma Stage 4 (MYCN Amplified)",
        },
        "subject": {
            "reference": "Patient/pat-sj-004",
        },
        "onsetDateTime": "2025-07-22",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-005",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "21708004",
                    "display": "Osteosarcoma",
                },
            ],
            "text": "Osteosarcoma of Distal Right Femur",
        },
        "subject": {
            "reference": "Patient/pat-sj-005",
        },
        "onsetDateTime": "2025-10-05",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-006",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "76310001",
                    "display": "Severe combined immunodeficiency",
                },
            ],
            "text": "Severe Combined Immunodeficiency (ADA Deficiency)",
        },
        "subject": {
            "reference": "Patient/pat-sj-006",
        },
        "onsetDateTime": "2024-03-10",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-007",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "118599009",
                    "display": "Hodgkin lymphoma",
                },
            ],
            "text": "Classical Hodgkin Lymphoma, Nodular Sclerosis Stage IIA",
        },
        "subject": {
            "reference": "Patient/pat-sj-007",
        },
        "onsetDateTime": "2026-01-18",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-008",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "370967009",
                    "display": "Retinoblastoma",
                },
            ],
            "text": "Bilateral Retinoblastoma (Germline RB1 Mutation)",
        },
        "subject": {
            "reference": "Patient/pat-sj-008",
        },
        "onsetDateTime": "2024-02-14",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-009",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "306058006",
                    "display": "Aplastic anemia",
                },
            ],
            "text": "Very Severe Aplastic Anemia (Idiopathic)",
        },
        "subject": {
            "reference": "Patient/pat-sj-009",
        },
        "onsetDateTime": "2025-12-01",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-010",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "190905008",
                    "display": "Cystic fibrosis",
                },
            ],
            "text": ("Cystic Fibrosis (delta-F508 homozygous) with Pancreatic Insufficiency"),
        },
        "subject": {
            "reference": "Patient/pat-sj-010",
        },
        "onsetDateTime": "2020-10-15",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-011",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "109989006",
                    "display": "T-cell acute lymphoblastic leukemia",
                },
            ],
            "text": "T-Cell Acute Lymphoblastic Leukemia",
        },
        "subject": {
            "reference": "Patient/pat-sj-011",
        },
        "onsetDateTime": "2026-03-10",
    },
    {
        "resourceType": "Condition",
        "id": "cond-sj-012",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "722212001",
                    "display": "Ewing sarcoma",
                },
            ],
            "text": "Ewing Sarcoma of Pelvis",
        },
        "subject": {
            "reference": "Patient/pat-sj-012",
        },
        "onsetDateTime": "2025-08-30",
    },
]

_MEDICATIONS: list[dict[str, Any]] = [
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-001",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "115113",
                    "display": "Vincristine 1 MG/ML Injectable Solution",
                },
            ],
            "text": "Vincristine Sulfate 1.5 mg/m2 IV",
        },
        "dosageInstruction": [
            {
                "text": "1.5 mg/m2 IV weekly on St. Jude Total Therapy Protocol XVII",
            },
        ],
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-002",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "6851",
                    "display": "Methotrexate",
                },
            ],
            "text": "High-Dose Methotrexate 5 g/m2 IV",
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-003",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-003",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "5521",
                    "display": "Hydroxyurea",
                },
            ],
            "text": "Hydroxyurea 20 mg/kg Oral Daily",
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-004",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "7617",
                    "display": "Ondansetron",
                },
            ],
            "text": "Ondansetron 4 mg IV q8h PRN nausea",
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-005",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "284687",
                    "display": "Pegfilgrastim",
                },
            ],
            "text": "Pegfilgrastim 6 mg SubQ once per cycle",
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-006",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-002",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "2555",
                    "display": "Cisplatin",
                },
            ],
            "text": "Cisplatin 75 mg/m2 IV",
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-007",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-004",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "1657866",
                    "display": "Dinutuximab",
                },
            ],
            "text": "Dinutuximab 17.5 mg/m2/day IV infusion",
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-008",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-005",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "3639",
                    "display": "Doxorubicin",
                },
            ],
            "text": "Doxorubicin 37.5 mg/m2 IV continuous infusion",
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-009",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-007",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "1588",
                    "display": "Bleomycin",
                },
            ],
            "text": "Bleomycin 10 units/m2 IV (ABVD Protocol)",
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-010",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-009",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "3008",
                    "display": "Cyclosporine",
                },
            ],
            "text": "Cyclosporine 5 mg/kg/day divided BID",
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-011",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-010",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "2200643",
                    "display": "Trikafta",
                },
            ],
            "text": ("Elexacaftor/Tezacaftor/Ivacaftor morning dose + Ivacaftor evening dose"),
        },
    },
    {
        "resourceType": "MedicationRequest",
        "id": "med-sj-012",
        "status": "active",
        "intent": "order",
        "subject": {
            "reference": "Patient/pat-sj-012",
        },
        "medicationCodeableConcept": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "5690",
                    "display": "Ifosfamide",
                },
            ],
            "text": "Ifosfamide 1800 mg/m2/day IV with Mesna uroprotection",
        },
    },
]

_ALLERGIES: list[dict[str, Any]] = [
    {
        "resourceType": "AllergyIntolerance",
        "id": "alg-sj-001",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "type": "allergy",
        "category": [
            "medication",
        ],
        "criticality": "high",
        "code": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "7980",
                    "display": "Penicillin G",
                },
            ],
            "text": "Penicillin Class Antibiotics",
        },
        "patient": {
            "reference": "Patient/pat-sj-001",
        },
        "reaction": [
            {
                "manifestation": [
                    {
                        "text": "Anaphylaxis and generalized urticaria",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "AllergyIntolerance",
        "id": "alg-sj-002",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "type": "allergy",
        "category": [
            "medication",
        ],
        "criticality": "high",
        "code": {
            "coding": [
                {
                    "system": "http://www.nlm.nih.gov/research/umls/rxnorm",
                    "code": "10180",
                    "display": "Sulfamethoxazole",
                },
            ],
            "text": "Sulfonamides (Bactrim/Septra)",
        },
        "patient": {
            "reference": "Patient/pat-sj-002",
        },
        "reaction": [
            {
                "manifestation": [
                    {
                        "text": "Stevens-Johnson Syndrome warning / Erythema multiforme",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "AllergyIntolerance",
        "id": "alg-sj-003",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "verificationStatus": {
            "coding": [
                {
                    "code": "confirmed",
                },
            ],
        },
        "category": [
            "environment",
        ],
        "criticality": "low",
        "code": {
            "text": "Latex",
        },
        "patient": {
            "reference": "Patient/pat-sj-003",
        },
        "reaction": [
            {
                "manifestation": [
                    {
                        "text": "Contact dermatitis",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "AllergyIntolerance",
        "id": "alg-sj-004",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "category": [
            "food",
        ],
        "criticality": "high",
        "code": {
            "text": "Peanuts",
        },
        "patient": {
            "reference": "Patient/pat-sj-004",
        },
        "reaction": [
            {
                "manifestation": [
                    {
                        "text": "Bronchospasm, facial edema",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "AllergyIntolerance",
        "id": "alg-sj-005",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "category": [
            "medication",
        ],
        "criticality": "medium",
        "code": {
            "text": "Morphine",
        },
        "patient": {
            "reference": "Patient/pat-sj-005",
        },
        "reaction": [
            {
                "manifestation": [
                    {
                        "text": "Severe pruritus and hypotensive flushing",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "AllergyIntolerance",
        "id": "alg-sj-006",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "category": [
            "medication",
        ],
        "criticality": "medium",
        "code": {
            "text": "Iodinated Radiocontrast Media",
        },
        "patient": {
            "reference": "Patient/pat-sj-007",
        },
        "reaction": [
            {
                "manifestation": [
                    {
                        "text": "Urticaria, pre-treatment required",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "AllergyIntolerance",
        "id": "alg-sj-007",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "category": [
            "medication",
        ],
        "criticality": "high",
        "code": {
            "text": "Ciprofloxacin",
        },
        "patient": {
            "reference": "Patient/pat-sj-010",
        },
        "reaction": [
            {
                "manifestation": [
                    {
                        "text": "Severe rash, facial swelling",
                    },
                ],
            },
        ],
    },
    {
        "resourceType": "AllergyIntolerance",
        "id": "alg-sj-008",
        "clinicalStatus": {
            "coding": [
                {
                    "code": "active",
                },
            ],
        },
        "category": [
            "medication",
        ],
        "criticality": "high",
        "code": {
            "text": "L-Asparaginase",
        },
        "patient": {
            "reference": "Patient/pat-sj-011",
        },
        "reaction": [
            {
                "manifestation": [
                    {
                        "text": "Acute clinical hypersensitivity, switched to Erwinia",
                    },
                ],
            },
        ],
    },
]

_DIAGNOSTIC_REPORTS: list[dict[str, Any]] = [
    {
        "resourceType": "DiagnosticReport",
        "id": "rep-sj-001",
        "status": "final",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "33717-0",
                    "display": "Bone Marrow Aspirate & Biopsy",
                },
            ],
            "text": "Bone Marrow Flow Cytometry & Cytogenetics Evaluation",
        },
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "effectiveDateTime": "2026-03-20T10:00:00Z",
        "conclusion": (
            "B-cell lymphoblastic leukemia with 88% blasts; positive for "
            "ETV6-RUNX1 fusion. Day 29 MRD negative (<0.01%)."
        ),
    },
    {
        "resourceType": "DiagnosticReport",
        "id": "rep-sj-002",
        "status": "final",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "24627-2",
                    "display": "MRI Brain with Contrast",
                },
            ],
            "text": "Neuro-Oncology Cranial MRI",
        },
        "subject": {
            "reference": "Patient/pat-sj-002",
        },
        "effectiveDateTime": "2026-03-15T15:30:00Z",
        "conclusion": (
            "Gross total resection cavity in fourth ventricle with no "
            "nodular recurrence or leptomeningeal enhancement."
        ),
    },
    {
        "resourceType": "DiagnosticReport",
        "id": "rep-sj-003",
        "status": "final",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "42247-7",
                    "display": "Hemoglobin Variant HPLC",
                },
            ],
            "text": "Hemoglobin Electrophoresis & HPLC Analysis",
        },
        "subject": {
            "reference": "Patient/pat-sj-003",
        },
        "effectiveDateTime": "2026-03-30T10:00:00Z",
        "conclusion": (
            "HbS 58.2%, HbF 24.1%, HbA2 3.1%, HbA 14.6% (post-transfusion "
            "status). High HbF confers protective benefit."
        ),
    },
    {
        "resourceType": "DiagnosticReport",
        "id": "rep-sj-004",
        "status": "final",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "72231-4",
                    "display": "MIBG Scan",
                },
            ],
            "text": "123I-mIBG Whole Body Scintigraphy",
        },
        "subject": {
            "reference": "Patient/pat-sj-004",
        },
        "effectiveDateTime": "2026-03-10T11:00:00Z",
        "conclusion": (
            "Significant resolution of skeletal metastases. Curie score decreased from 16 to 2."
        ),
    },
    {
        "resourceType": "DiagnosticReport",
        "id": "rep-sj-005",
        "status": "final",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "36813-4",
                    "display": "MRI Right Femur",
                },
            ],
            "text": "Diagnostic Musculoskeletal MRI Right Lower Extremity",
        },
        "subject": {
            "reference": "Patient/pat-sj-005",
        },
        "effectiveDateTime": "2026-02-28T09:00:00Z",
        "conclusion": (
            "Good histological response (>90% necrosis predicted) "
            "following neoadjuvant MAP chemotherapy."
        ),
    },
    {
        "resourceType": "DiagnosticReport",
        "id": "rep-sj-006",
        "status": "final",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "49547-3",
                    "display": "Lymphocyte Subsets Panel",
                },
            ],
            "text": "Comprehensive Immune Reconstitution Flow Cytometry",
        },
        "subject": {
            "reference": "Patient/pat-sj-006",
        },
        "effectiveDateTime": "2026-03-15T09:30:00Z",
        "conclusion": (
            "Robust donor-derived T-cell and B-cell immune reconstitution "
            "following retroviral gene therapy."
        ),
    },
    {
        "resourceType": "DiagnosticReport",
        "id": "rep-sj-007",
        "status": "final",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "44136-0",
                    "display": "PET-CT Whole Body",
                },
            ],
            "text": "18F-FDG PET/CT Lymphoma Restaging",
        },
        "subject": {
            "reference": "Patient/pat-sj-007",
        },
        "effectiveDateTime": "2026-03-01T14:00:00Z",
        "conclusion": (
            "Deauville score 1 (complete metabolic remission) after two "
            "cycles of OEPA chemotherapy."
        ),
    },
    {
        "resourceType": "DiagnosticReport",
        "id": "rep-sj-008",
        "status": "final",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "72231-4",
                    "display": "Chest CT",
                },
            ],
            "text": "High Resolution Chest CT",
        },
        "subject": {
            "reference": "Patient/pat-sj-010",
        },
        "effectiveDateTime": "2026-01-20T10:00:00Z",
        "conclusion": (
            "Mild bronchiectasis in right middle lobe. No acute consolidations or mucus plugging."
        ),
    },
]

_ENCOUNTERS: list[dict[str, Any]] = [
    {
        "resourceType": "Encounter",
        "id": "enc-sj-001",
        "status": "in-progress",
        "class": {
            "code": "IMP",
            "display": "inpatient encounter",
        },
        "type": [
            {
                "text": ("Pediatric Oncology Inpatient Chemotherapy & Febrile Neutropenia"),
            },
        ],
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "period": {
            "start": "2026-04-01T08:00:00Z",
        },
    },
    {
        "resourceType": "Encounter",
        "id": "enc-sj-002",
        "status": "finished",
        "class": {
            "code": "AMB",
            "display": "ambulatory",
        },
        "type": [
            {
                "text": "Outpatient Neuro-Oncology Follow-up & Lumbar Puncture",
            },
        ],
        "subject": {
            "reference": "Patient/pat-sj-002",
        },
        "period": {
            "start": "2026-03-28T13:00:00Z",
            "end": "2026-03-28T16:00:00Z",
        },
    },
    {
        "resourceType": "Encounter",
        "id": "enc-sj-003",
        "status": "finished",
        "class": {
            "code": "EMER",
            "display": "emergency",
        },
        "type": [
            {
                "text": "Emergency Department Vaso-Occlusive Pain Crisis Management",
            },
        ],
        "subject": {
            "reference": "Patient/pat-sj-003",
        },
        "period": {
            "start": "2026-03-29T22:00:00Z",
            "end": "2026-03-30T14:00:00Z",
        },
    },
    {
        "resourceType": "Encounter",
        "id": "enc-sj-004",
        "status": "in-progress",
        "class": {
            "code": "IMP",
            "display": "inpatient encounter",
        },
        "type": [
            {
                "text": "Immunotherapy & Dinutuximab Continuous Infusion",
            },
        ],
        "subject": {
            "reference": "Patient/pat-sj-004",
        },
        "period": {
            "start": "2026-03-25T08:00:00Z",
        },
    },
    {
        "resourceType": "Encounter",
        "id": "enc-sj-005",
        "status": "finished",
        "class": {
            "code": "AMB",
            "display": "ambulatory",
        },
        "type": [
            {
                "text": "Pre-Surgical Limb-Salvage Planning Clinic",
            },
        ],
        "subject": {
            "reference": "Patient/pat-sj-005",
        },
        "period": {
            "start": "2026-03-27T09:00:00Z",
            "end": "2026-03-27T12:00:00Z",
        },
    },
    {
        "resourceType": "Encounter",
        "id": "enc-sj-006",
        "status": "finished",
        "class": {
            "code": "AMB",
            "display": "ambulatory",
        },
        "type": [
            {
                "text": "Post-Gene Therapy Longitudinal Surveillance",
            },
        ],
        "subject": {
            "reference": "Patient/pat-sj-006",
        },
        "period": {
            "start": "2026-03-15T09:00:00Z",
            "end": "2026-03-15T11:30:00Z",
        },
    },
    {
        "resourceType": "Encounter",
        "id": "enc-sj-007",
        "status": "finished",
        "class": {
            "code": "AMB",
            "display": "ambulatory",
        },
        "type": [
            {
                "text": "Lymphoma Infusion Center Chemotherapy Session",
            },
        ],
        "subject": {
            "reference": "Patient/pat-sj-007",
        },
        "period": {
            "start": "2026-03-20T08:30:00Z",
            "end": "2026-03-20T13:00:00Z",
        },
    },
    {
        "resourceType": "Encounter",
        "id": "enc-sj-008",
        "status": "finished",
        "class": {
            "code": "AMB",
            "display": "ambulatory",
        },
        "type": [
            {
                "text": "Pediatric Pulmonology Comprehensive CF Clinic",
            },
        ],
        "subject": {
            "reference": "Patient/pat-sj-010",
        },
        "period": {
            "start": "2026-01-20T09:00:00Z",
            "end": "2026-01-20T12:00:00Z",
        },
    },
]

_CARE_PLANS: list[dict[str, Any]] = [
    {
        "resourceType": "CarePlan",
        "id": "cp-sj-001",
        "status": "active",
        "intent": "plan",
        "title": "St. Jude Total Therapy Protocol XVII for B-ALL",
        "subject": {
            "reference": "Patient/pat-sj-001",
        },
        "category": [
            {
                "text": "Pediatric Leukemia Clinical Trial Protocol",
            },
        ],
        "description": (
            "Standard-risk arm: Remission induction, consolidation, and maintenance therapy."
        ),
    },
    {
        "resourceType": "CarePlan",
        "id": "cp-sj-002",
        "status": "active",
        "intent": "plan",
        "title": "PBTC Protocol for WNT Medulloblastoma",
        "subject": {
            "reference": "Patient/pat-sj-002",
        },
        "category": [
            {
                "text": "Pediatric Neuro-Oncology Care Plan",
            },
        ],
        "description": ("Reduced-dose craniospinal irradiation followed by chemotherapy."),
    },
    {
        "resourceType": "CarePlan",
        "id": "cp-sj-003",
        "status": "active",
        "intent": "plan",
        "title": "Comprehensive Sickle Cell Disease Disease-Modifying Plan",
        "subject": {
            "reference": "Patient/pat-sj-003",
        },
        "category": [
            {
                "text": "Hematology Care Plan",
            },
        ],
        "description": (
            "Maximum tolerated dose hydroxyurea, annual transcranial "
            "Doppler screening, and iron chelation."
        ),
    },
    {
        "resourceType": "CarePlan",
        "id": "cp-sj-004",
        "status": "active",
        "intent": "plan",
        "title": "High-Risk Neuroblastoma ANBL0032 Post-Consolidation Protocol",
        "subject": {
            "reference": "Patient/pat-sj-004",
        },
        "category": [
            {
                "text": "Pediatric Oncology Care Plan",
            },
        ],
        "description": ("Dinutuximab + GM-CSF + Isotretinoin maintenance immunotherapy."),
    },
    {
        "resourceType": "CarePlan",
        "id": "cp-sj-005",
        "status": "active",
        "intent": "plan",
        "title": "Multi-Agent MAP Protocol for Localized Osteosarcoma",
        "subject": {
            "reference": "Patient/pat-sj-005",
        },
        "category": [
            {
                "text": "Pediatric Solid Tumor Care Plan",
            },
        ],
        "description": (
            "Neoadjuvant high-dose methotrexate, doxorubicin, cisplatin; "
            "limb-salvage resection; adjuvant cycles."
        ),
    },
    {
        "resourceType": "CarePlan",
        "id": "cp-sj-006",
        "status": "active",
        "intent": "plan",
        "title": "Cystic Fibrosis Pulmonary and Nutritional Optimization Plan",
        "subject": {
            "reference": "Patient/pat-sj-010",
        },
        "category": [
            {
                "text": "Chronic Pulmonary Care Plan",
            },
        ],
        "description": (
            "CFTR modulator therapy, daily airway clearance, pancreatic "
            "enzyme replacement, fat-soluble vitamins."
        ),
    },
]


# ---------------------------------------------------------------------------
# ClinicalSandbox Implementation
# ---------------------------------------------------------------------------


class ClinicalSandbox:
    """In-memory clinical FHIR sandbox with rich synthetic data from St. Jude Research Institute.

    All mutating methods enforce strict safety invariants (assert_write_permitted)
    and restrict creation to draft states.
    """

    def __init__(self) -> None:
        self._patients: list[dict[str, Any]] = []
        self._observations: list[dict[str, Any]] = []
        self._conditions: list[dict[str, Any]] = []
        self._medications: list[dict[str, Any]] = []
        self._allergies: list[dict[str, Any]] = []
        self._diagnostic_reports: list[dict[str, Any]] = []
        self._encounters: list[dict[str, Any]] = []
        self._care_plans: list[dict[str, Any]] = []
        self.reset()

    def reset(self) -> None:
        """Reset the sandbox to the pristine initial synthetic dataset."""
        self._patients = copy.deepcopy(_PATIENTS)
        self._observations = copy.deepcopy(_OBSERVATIONS)
        self._conditions = copy.deepcopy(_CONDITIONS)
        self._medications = copy.deepcopy(_MEDICATIONS)
        self._allergies = copy.deepcopy(_ALLERGIES)
        self._diagnostic_reports = copy.deepcopy(_DIAGNOSTIC_REPORTS)
        self._encounters = copy.deepcopy(_ENCOUNTERS)
        self._care_plans = copy.deepcopy(_CARE_PLANS)

    # -----------------------------------------------------------------------
    # Bundle Helper
    # -----------------------------------------------------------------------

    def to_bundle(
        self, resources: list[dict[str, Any]], total: int | None = None
    ) -> dict[str, Any]:
        """Wrap a list of FHIR resources in a standard searchset Bundle."""
        return {
            "resourceType": "Bundle",
            "type": "searchset",
            "total": total if total is not None else len(resources),
            "entry": [{"resource": res} for res in resources],
        }

    # -----------------------------------------------------------------------
    # Patient Queries
    # -----------------------------------------------------------------------

    def search_patients(
        self,
        query: str | None = None,
        name: str | None = None,
        identifier: str | None = None,
        gender: str | None = None,
        count: int | None = None,
    ) -> list[dict[str, Any]]:
        """Search synthetic patients with flexible query filtering."""
        results: list[dict[str, Any]] = []
        for pat in self._patients:
            if gender and pat.get("gender") != gender.lower():
                continue

            if identifier:
                pat_idents = [ident.get("value", "") for ident in pat.get("identifier", [])]
                if identifier not in pat_idents:
                    continue

            if name:
                pat_names = []
                for n in pat.get("name", []):
                    pat_names.extend(n.get("given", []))
                    if "family" in n:
                        pat_names.append(n["family"])
                    if "text" in n:
                        pat_names.append(n["text"])
                if not any(name.lower() in str(val).lower() for val in pat_names):
                    continue

            if query:
                q_lower = query.lower()
                matched = False
                if q_lower in pat.get("id", "").lower():
                    matched = True
                for n in pat.get("name", []):
                    if q_lower in str(n.get("family", "")).lower():
                        matched = True
                    for g in n.get("given", []):
                        if q_lower in str(g).lower():
                            matched = True
                    if q_lower in str(n.get("text", "")).lower():
                        matched = True
                for ident in pat.get("identifier", []):
                    if q_lower in str(ident.get("value", "")).lower():
                        matched = True
                if not matched:
                    continue

            results.append(copy.deepcopy(pat))

        if count is not None and count > 0:
            results = results[:count]

        return results

    def get_patient(self, patient_id: str) -> dict[str, Any] | None:
        """Fetch a single Patient by ID, or None if not found."""
        clean_id = patient_id.replace("Patient/", "").strip()
        for pat in self._patients:
            if pat.get("id") == clean_id:
                return copy.deepcopy(pat)
        return None

    # -----------------------------------------------------------------------
    # Observation Methods
    # -----------------------------------------------------------------------

    def list_observations(
        self,
        patient_id: str | None = None,
        category: str | None = None,
        code: str | None = None,
        status: str | None = None,
        count: int | None = None,
    ) -> list[dict[str, Any]]:
        """List and filter observations."""
        results: list[dict[str, Any]] = []
        clean_pat_id = patient_id.replace("Patient/", "").strip() if patient_id else None

        for obs in self._observations:
            if clean_pat_id:
                ref = obs.get("subject", {}).get("reference", "")
                if ref.replace("Patient/", "").strip() != clean_pat_id:
                    continue

            if status and obs.get("status") != status:
                continue

            if code:
                obs_codes: list[str] = []
                for coding in obs.get("code", {}).get("coding", []):
                    obs_codes.append(coding.get("code", ""))
                if "text" in obs.get("code", {}):
                    obs_codes.append(obs["code"]["text"])
                if not any(code.lower() in c.lower() for c in obs_codes):
                    continue

            results.append(copy.deepcopy(obs))

        if count is not None and count > 0:
            results = results[:count]

        return results

    def get_observation(self, observation_id: str) -> dict[str, Any] | None:
        """Fetch a single Observation by ID."""
        clean_id = observation_id.replace("Observation/", "").strip()
        for obs in self._observations:
            if obs.get("id") == clean_id:
                return copy.deepcopy(obs)
        return None

    def create_observation_draft(self, resource: dict[str, Any]) -> dict[str, Any]:
        """Create a draft Observation, enforcing draft invariants."""
        # Non-bypassable safety gate
        assert_write_permitted("Observation", resource, allow_writes=True)

        stored = copy.deepcopy(resource)
        stored["resourceType"] = "Observation"
        if "id" not in stored or not stored["id"]:
            stored["id"] = f"obs-draft-{uuid.uuid4().hex[:8]}"

        if "status" not in stored:
            stored["status"] = "registered"

        self._observations.append(stored)
        return copy.deepcopy(stored)

    # -----------------------------------------------------------------------
    # Condition Methods
    # -----------------------------------------------------------------------

    def list_conditions(
        self,
        patient_id: str | None = None,
        category: str | None = None,
        clinical_status: str | None = None,
        count: int | None = None,
    ) -> list[dict[str, Any]]:
        """List and filter conditions."""
        results: list[dict[str, Any]] = []
        clean_pat_id = patient_id.replace("Patient/", "").strip() if patient_id else None

        for cond in self._conditions:
            if clean_pat_id:
                ref = cond.get("subject", {}).get("reference", "")
                if ref.replace("Patient/", "").strip() != clean_pat_id:
                    continue
            results.append(copy.deepcopy(cond))

        if count is not None and count > 0:
            results = results[:count]

        return results

    def get_condition(self, condition_id: str) -> dict[str, Any] | None:
        """Fetch a single Condition by ID."""
        clean_id = condition_id.replace("Condition/", "").strip()
        for cond in self._conditions:
            if cond.get("id") == clean_id:
                return copy.deepcopy(cond)
        return None

    # -----------------------------------------------------------------------
    # MedicationRequest Methods
    # -----------------------------------------------------------------------

    def list_medications(
        self,
        patient_id: str | None = None,
        status: str | None = None,
        count: int | None = None,
    ) -> list[dict[str, Any]]:
        """List and filter MedicationRequests."""
        results: list[dict[str, Any]] = []
        clean_pat_id = patient_id.replace("Patient/", "").strip() if patient_id else None

        for med in self._medications:
            if clean_pat_id:
                ref = med.get("subject", {}).get("reference", "")
                if ref.replace("Patient/", "").strip() != clean_pat_id:
                    continue
            if status and med.get("status") != status:
                continue
            results.append(copy.deepcopy(med))

        if count is not None and count > 0:
            results = results[:count]

        return results

    def get_medication(self, medication_id: str) -> dict[str, Any] | None:
        """Fetch a single MedicationRequest by ID."""
        clean_id = medication_id.replace("MedicationRequest/", "").strip()
        for med in self._medications:
            if med.get("id") == clean_id:
                return copy.deepcopy(med)
        return None

    def create_medication_draft(self, resource: dict[str, Any]) -> dict[str, Any]:
        """Create a draft MedicationRequest, enforcing draft invariants."""
        # Non-bypassable safety gate
        assert_write_permitted("MedicationRequest", resource, allow_writes=True)

        stored = copy.deepcopy(resource)
        stored["resourceType"] = "MedicationRequest"
        if "id" not in stored or not stored["id"]:
            stored["id"] = f"med-draft-{uuid.uuid4().hex[:8]}"

        if "status" not in stored:
            stored["status"] = "draft"
        if "intent" not in stored:
            stored["intent"] = "order"

        self._medications.append(stored)
        return copy.deepcopy(stored)

    # -----------------------------------------------------------------------
    # AllergyIntolerance Methods
    # -----------------------------------------------------------------------

    def list_allergies(
        self,
        patient_id: str | None = None,
        count: int | None = None,
    ) -> list[dict[str, Any]]:
        """List and filter AllergyIntolerance resources."""
        results: list[dict[str, Any]] = []
        clean_pat_id = patient_id.replace("Patient/", "").strip() if patient_id else None

        for allg in self._allergies:
            if clean_pat_id:
                ref = allg.get("patient", {}).get("reference", "")
                if ref.replace("Patient/", "").strip() != clean_pat_id:
                    continue
            results.append(copy.deepcopy(allg))

        if count is not None and count > 0:
            results = results[:count]

        return results

    def get_allergy(self, allergy_id: str) -> dict[str, Any] | None:
        """Fetch a single AllergyIntolerance by ID."""
        clean_id = allergy_id.replace("AllergyIntolerance/", "").strip()
        for allg in self._allergies:
            if allg.get("id") == clean_id:
                return copy.deepcopy(allg)
        return None

    # -----------------------------------------------------------------------
    # DiagnosticReport Methods
    # -----------------------------------------------------------------------

    def list_diagnostic_reports(
        self,
        patient_id: str | None = None,
        code: str | None = None,
        count: int | None = None,
    ) -> list[dict[str, Any]]:
        """List and filter DiagnosticReports."""
        results: list[dict[str, Any]] = []
        clean_pat_id = patient_id.replace("Patient/", "").strip() if patient_id else None

        for rep in self._diagnostic_reports:
            if clean_pat_id:
                ref = rep.get("subject", {}).get("reference", "")
                if ref.replace("Patient/", "").strip() != clean_pat_id:
                    continue
            results.append(copy.deepcopy(rep))

        if count is not None and count > 0:
            results = results[:count]

        return results

    def get_diagnostic_report(self, report_id: str) -> dict[str, Any] | None:
        """Fetch a single DiagnosticReport by ID."""
        clean_id = report_id.replace("DiagnosticReport/", "").strip()
        for rep in self._diagnostic_reports:
            if rep.get("id") == clean_id:
                return copy.deepcopy(rep)
        return None

    # -----------------------------------------------------------------------
    # Encounter & CarePlan Methods
    # -----------------------------------------------------------------------

    def list_encounters(
        self,
        patient_id: str | None = None,
        status: str | None = None,
        count: int | None = None,
    ) -> list[dict[str, Any]]:
        """List and filter encounters."""
        results: list[dict[str, Any]] = []
        clean_pat_id = patient_id.replace("Patient/", "").strip() if patient_id else None

        for enc in self._encounters:
            if clean_pat_id:
                ref = enc.get("subject", {}).get("reference", "")
                if ref.replace("Patient/", "").strip() != clean_pat_id:
                    continue
            if status and enc.get("status") != status:
                continue
            results.append(copy.deepcopy(enc))

        if count is not None and count > 0:
            results = results[:count]

        return results

    def get_encounter(self, encounter_id: str) -> dict[str, Any] | None:
        """Fetch a single Encounter by ID."""
        clean_id = encounter_id.replace("Encounter/", "").strip()
        for enc in self._encounters:
            if enc.get("id") == clean_id:
                return copy.deepcopy(enc)
        return None

    def list_care_plans(
        self,
        patient_id: str | None = None,
        status: str | None = None,
        count: int | None = None,
    ) -> list[dict[str, Any]]:
        """List and filter CarePlans."""
        results: list[dict[str, Any]] = []
        clean_pat_id = patient_id.replace("Patient/", "").strip() if patient_id else None

        for cp in self._care_plans:
            if clean_pat_id:
                ref = cp.get("subject", {}).get("reference", "")
                if ref.replace("Patient/", "").strip() != clean_pat_id:
                    continue
            if status and cp.get("status") != status:
                continue
            results.append(copy.deepcopy(cp))

        if count is not None and count > 0:
            results = results[:count]

        return results

    def get_care_plan(self, care_plan_id: str) -> dict[str, Any] | None:
        """Fetch a single CarePlan by ID."""
        clean_id = care_plan_id.replace("CarePlan/", "").strip()
        for cp in self._care_plans:
            if cp.get("id") == clean_id:
                return copy.deepcopy(cp)
        return None
