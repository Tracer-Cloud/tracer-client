Tracer Onboarding Experience and Documentation 
# Prerequisites Setup

## Install Microsoft Visual Studio Code & Git

1. Go to [Visual Studio Microsoft] (https://visualstudio.microsoft.com/downloads/?cid=learn-navbar-download-cta)
     
2. Download the free version which suits your machine: 
        
3. Follow the steps in order to complete the download and the setup

4. Install Git: 
        
    a/ Click the button ‘Install Git’ after which the following link opens in your browser [Installing Git] (https://git-scm.com/book/en/v2/Getting-Started-Installing-Git)
    
    b/  Scroll to your operating system and follow the instructions in order to install Git.
    
5. Clone the Tracer GitHub repository
    
    ```markdown
    git clone
    ```
    
    a/ Do this by clicking on the button ‘Clone Repository’
        
    b/ After which a search bar will appear and in this search bar, you paste the following URL [Github-Tracer] (https://github.com/Tracer-Cloud/tracer-client.git)
        
    c/ Then you will have to choose a location to store the Tracer GitHub repository, remember this location as you will need it again further down this process
    
    d/ Select to trust authors 
    
    e/ When all previous steps have been performed correctly, you should end up with a view which let's you start/open a new file
    


## Install Terraform on your local device

### Install Homebrew

1. Go to the following link [Homebrew] (https://brew.sh/)
    
2. Here you will find a line of code that you can paste in a macOS Terminal or Linux shell prompt:

```markdown
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

3. After which you should have the following display asking for your device’s password.

4. Fill in your password and press ‘ENTER’
    
5. After your password is filled in correctly, you should see a code asking to press enter if you want to continue.
    
6. Press ‘ENTER’ again, after which Homebrew will start to download.
        
7. It is important to go through the text provided during this download as it states ‘Next steps’, which states 3 lines of commands to run in your terminal in order to add Homebrew to your path which is crucial for a completed installation. The three lines of code start with:
    
    ```markdown
    echo >>
    echo ‘eval’
    eval “
    ```
    
    → You can find these lines as line 4 to 6 counting from the bottom up
    
8. After completing step 7, you should see a confirmation screen that shows Homebrew is now added to your path:



### Install Terraform

1. Go to the following link [Terraform Developer] (https://developer.hashicorp.com/terraform/install?product_intent=terraform)
    
2. Scroll down to your device’s operating system and copy the code which is provided to download Terraform. Do not click on the ‘Binary Download’ options.
    
3. After completion of step 2, you should see a line of # after which 100% can be found.
  Terraform should hereby be installed on your device.
    

### Configure AWS CLI with the appropriate credentials

We will install AWS CLI via Homebrew

— These are instructions for macOS and Linux

1. Open up your Terminal and run the following code
    
    ```markdown
    brew install awscli
    ```    

2. Type the following line of code: 
    
    ```markdown
    aws --version
    ```

3. Type the following line of code: 
    
    ```markdown
    aws configure
    ```    

4. At this moment you are requested to provide your appropriate AWS credentials, fill in the first one and press ‘ENTER’ to let the next credential show:
    
    a/ AWS Access Key ID
    
    → Log in to the AWS Management Console > IAM > Users > Select appropriate username > View the Summary or click on Security Credentials to find your two Access Keys
    
    b/ AWS Secret Access Key
    
    → Log in to the AWS Management Console > IAM > Users > Select appropriate username > View the Summary or click on Security Credentials to find your two Access Keys
    
    c/ Default region name
    
      Visible in the upper right corner, just left of your username
    
        ***!!!Your default region name should be filled in as the region code!!!***
        Region	Region Code	Region	Region Code
        US East (N. Virginia)	      us-east-1	        |  Canada (Central)	          ca-central-1
        US East (Ohio)	            us-east-2	        |  Europe (Frankfurt)	        eu-central-1
        US West (N. California)	    us-west-1	        |  Europe (Ireland)	          eu-west-1
        US West (Oregon)	          us-west-2	        |  Europe (London)	          eu-west-2
        Asia Pacific (Hong Kong)	  ap-east-1	        |  Europe (Milan)	            eu-south-1
        Asia Pacific (Mumbai)	      ap-south-1	      |  Europe (Paris)	            eu-west-3
        Asia Pacific (Osaka-Local)	ap-northeast-3	  |  Europe (Stockholm)	        eu-north-1
        Asia Pacific (Seoul)	      ap-northeast-2	  |  South America (São Paulo)	sa-east-1
        Asia Pacific (Singapore)	  ap-southeast-1	  |  Middle East (Bahrain)	    me-south-1
        Asia Pacific (Sydney)	      ap-southeast-2	  |  Africa (Cape Town)	        af-south-1
        Asia Pacific (Tokyo)	      ap-northeast-1	

d/ Default output format
     Should always be JSON

2. If all four are filled in, it is wise to double check if all credentials are successfully recorded. You can do this using the following line of code: 
    
    ```markdown
    aws s3 ls
    ```
    
    a/ If something is wrong, you will be notified by the output to this code. It will state which AWS credential could cause an error.
        
    b/ If everything is correct, you should receive this as last line of code as response to the above line of code: tracer-releases      

At this point, the Prerequisites Setup is completed and you can progress to the second topic.

---

---

# Deployment Process

1. Navigate to the infrastructure directory
    
    ```markdown
    cd path/tracer-client/infrastructure/one-command-infra-provisioning
    ```
    
    ***!IMPORTANT! ‘Path’*** is the location where you saved the ‘Tracer GitHub repository’ in step 5(c) of “Install Microsoft Visual Studio Code & Git
    e.g. if you saved it on in your ‘Documents’ file, your line of code becomes:                                  
    
    ```markdown
    cd documents/tracer-client/infrastructure/one-command-infra-provisioning
    ```
    
2. Now, we can initialize Terraform:
    
    ```markdown
    terraform init
    ```
    
    a/ If you get the a screen that states 'Terraform has been successfully initialized!' all is good and you can proceed to step 3 of this process
        
    b/ If you get the a screen that states 'Terraform initialized in an empty directory', you most probably have to open up another file before being able to initialize Terraform: 
        
    To see all options within the infrastructure file, enter the following code:
    
    ```markdown
    ls
    ```
    
    and you should see a list of possible files to select
        
    Now open up the launch-template file by entering:
    
    ```markdown
    cd launch-template
    ```
    
    Hereafter, you can restart step 2 from the beginning.
    
3. Review the Terraform configuration files to understand what will be deployed
    
4. Deploy the infrastructure by coding:
    
    ```markdown
    terraform apply
    ```
    
    By running the above line of code, a new workspace is automatically created in Amazon Managed Grafana, and a new launch template is generated as well.
        
    Now at the end of the response to the above code a command is given 'Enter a value:', if you would like to continue the deployment of Terraform, type the value
    
    ```markdown
    yes
    ```
    
    in the promt and press ‘ENTER’
    
5. After the completion of step 4, you should see a following screen that states 'Apply complete!'
    
    
    This indicates a successful deployment and the end of the deployment process.
    

Now you can proceed to Chapter 3. Sandbox Configuration.

---

---

# Sandbox Configuration

## Install Microsoft Visual Studio Code & Git

1. Access the newly deployed [Grafana Dashboard] (https://g-3f84880db9.grafana-workspace.us-east-1.amazonaws.com/dashboards)
    
2. Go to AWS
    
3. Open EC2  
    
4. Go to Instances  
    
5. Here you see the pipeline you just created in Terminal
        
6. Select the instance, by selecting the square and click the ‘CONNECT’ button on top
        
7. In the new screen, click ‘CONNECT’ again at the bottom in orange
        
8. Another tab opens and you should see a terminal in where you can code.
    
9. Enter the code
    
    ```markdown
    tracer
    ```
  
    and press ‘ENTER’


10.  Enter the code
    
    ```markdown
    tracer info
    ```
    
    and press ‘ENTER’    

    
11. Enter the code
    
    ```markdown
    tracer init --pipeline-name ABC
    ```
    
    → ***ABC*** is a replaceable name, you can give it any name you prefer
    
    and press ‘ENTER’


12.  You can use the link provided in this code to go to the Grafana Dashboards, which is the same as the one provided in step 1 of ‘3. Sandbox Configuration’
    
    
13.  When opening this link, you should see a welcome statement with options on the lefthand side.
    
14.  On the lefthand side, you can click on ‘Dashboards’ in order to see all dashboards.
        
15.  Select a preferred dashboard and it will open.
        
16.  Click a specific line for it to open one level deeper.
    







