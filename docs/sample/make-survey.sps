* Makes survey.sav, the sample file of the Statscraft guides.
* An invented customer survey of a fictional home internet provider.
* Every value is drawn at random, so no real person is in it.
* Run it from this folder: statscraft run make-survey.sps
* SET SEED makes the same file on every run.

SET SEED=20261123.

INPUT PROGRAM.
LOOP #i = 1 TO 300.
COMPUTE id = #i.
END CASE.
END LOOP.
END FILE.
END INPUT PROGRAM.

* Who answered.
COMPUTE region = 1 + TRUNC(RV.UNIFORM(0, 4)).
COMPUTE #g = RV.UNIFORM(0, 1).
RECODE #g (LOWEST THRU 0.51 = 1) (0.51 THRU 0.98 = 2) (ELSE = 3) INTO gender.
COMPUTE age = RND(RV.NORMAL(46, 15)).
IF (age < 18) age = 18 + TRUNC(RV.UNIFORM(0, 8)).
IF (age > 84) age = 84.
RECODE age (LOWEST THRU 29 = 1) (30 THRU 44 = 2) (45 THRU 59 = 3) (60 THRU HIGHEST = 4) INTO age_group.
COMPUTE education = 1 + TRUNC(RV.UNIFORM(0, 4)).
IF (age_group = 1 AND education = 4 AND RV.UNIFORM(0, 1) < 0.4) education = 2.

* Weights that bring the sample in line with the customer base:
* younger customers and men were less likely to answer.
COMPUTE weight = 1.
IF (age_group = 1) weight = 1.35.
IF (age_group = 4) weight = 0.8.
IF (gender = 2) weight = weight * 1.1.
COMPUTE weight = RND(weight * RV.UNIFORM(0.9, 1.1) * 100) / 100.

* Overall satisfaction, a little higher in the North and lower in the East.
COMPUTE #s = RV.NORMAL(3.3, 1.25).
IF (region = 1) #s = #s + 0.5.
IF (region = 3) #s = #s - 0.6.
COMPUTE satisfaction = RND(#s).
IF (satisfaction < 1) satisfaction = 1.
IF (satisfaction > 5) satisfaction = 5.
IF (RV.UNIFORM(0, 1) < 0.06) satisfaction = 9.

* Three yes or no questions. Those who did not know how satisfied they are
* answer as a neutral customer would.
COMPUTE #sat = satisfaction.
IF (satisfaction = 9) #sat = 3.
COMPUTE recommend = RV.UNIFORM(0, 1) < 0.15 + 0.15 * #sat.
COMPUTE contacted = RV.UNIFORM(0, 1) < 0.45.
COMPUTE switching = RV.UNIFORM(0, 1) < 0.55 - 0.09 * #sat.

* Channels used to reach the provider, one variable per channel.
COMPUTE ch_web = RV.UNIFORM(0, 1) < 0.7.
COMPUTE ch_app = RV.UNIFORM(0, 1) < 0.75 - 0.01 * age.
COMPUTE ch_phone = RV.UNIFORM(0, 1) < 0.15 + 0.008 * age.
COMPUTE ch_store = RV.UNIFORM(0, 1) < 0.2.
COMPUTE ch_email = RV.UNIFORM(0, 1) < 0.35.

* Monthly spend in euros. A few customers would not say.
COMPUTE spend = RND(EXP(RV.NORMAL(3.6, 0.35)) * 100) / 100.
IF (RV.UNIFORM(0, 1) < 0.04) spend = $SYSMIS.

* Interviews ran for six weeks from 5 October 2026.
COMPUTE interview_date = DATE.DMY(5, 10, 2026) + TRUNC(RV.UNIFORM(0, 42)) * 86400.

FORMATS id (F3.0) region gender age_group education satisfaction (F1.0) age (F2.0)
  recommend contacted switching ch_web ch_app ch_phone ch_store ch_email (F1.0)
  weight (F4.2) spend (F6.2) interview_date (DATE11).

VARIABLE LABELS
  id 'Respondent number'
  region 'Region'
  gender 'Gender'
  age 'Age in years'
  age_group 'Age group'
  education 'Highest education'
  weight 'Survey weight'
  satisfaction 'Overall satisfaction with the service'
  recommend 'Would recommend us to a friend'
  contacted 'Contacted customer support in the last year'
  switching 'Thinking about changing provider'
  ch_web 'Website'
  ch_app 'Mobile app'
  ch_phone 'Phone'
  ch_store 'Store'
  ch_email 'Email'
  spend 'Monthly spend in euros'
  interview_date 'Date of interview'.

VALUE LABELS region 1 'North' 2 'South' 3 'East' 4 'West'
  /gender 1 'Woman' 2 'Man' 3 'Other'
  /age_group 1 '18-29' 2 '30-44' 3 '45-59' 4 '60 and over'
  /education 1 'Basic' 2 'Secondary' 3 'Vocational' 4 'University'
  /satisfaction 1 'Very dissatisfied' 2 'Dissatisfied' 3 'Neutral' 4 'Satisfied'
    5 'Very satisfied' 9 'Don''t know'
  /recommend contacted switching 0 'No' 1 'Yes'
  /ch_web ch_app ch_phone ch_store ch_email 0 'Not used' 1 'Used'.

MISSING VALUES satisfaction (9).

VARIABLE LEVEL id region gender education (NOMINAL)
  age_group satisfaction (ORDINAL)
  recommend contacted switching ch_web ch_app ch_phone ch_store ch_email (NOMINAL)
  age weight spend interview_date (SCALE).

MRSETS /MDGROUP NAME=$channels LABEL='Channels used'
  VARIABLES=ch_web ch_app ch_phone ch_store ch_email VALUE=1.

SAVE OUTFILE='survey.sav'.
